use anyhow::{Context, Result};
use trafik_runtime::{FlowKey, Report};
use lazy_static::lazy_static;
use libbpf_rs::{Link, MapCore, MapFlags, Object, ObjectBuilder, RingBufferBuilder};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FlowEvent {
    event_type: u8,
    _pad: [u8; 3],
    flow: FlowKey,
    init_cwnd: u32,
    mss: u32,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct FlowStatistics {
    pub packets_in_flight: u32,
    pub bytes_in_flight: u32,
    pub bytes_pending: u32,
    pub rtt_sample_us: u32,
    pub was_timeout: u8,
    pub _pad1: [u8; 3],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct AckStatistics {
    pub bytes_acked: u32,
    pub packets_acked: u32,
    pub bytes_misordered: u32,
    pub packets_misordered: u32,
    pub ecn_bytes: u32,
    pub ecn_packets: u32,
    pub lost_pkts_sample: u32,
    pub now: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct Measurement {
    pub flow: FlowKey,
    pub rates: FlowRates,
    pub flow_stats: FlowStatistics,
    pub ack_stats: AckStatistics,
    pub snd_cwnd: u32,
    pub snd_ssthresh: u32,
    pub pacing_rate: u64,
    pub ca_state: u8,
    pub measurement_type: u8,
    pub _pad: [u8; 2],
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct FlowRates {
    pub rate_incoming: u32,
    pub rate_outgoing: u32,
}

impl Measurement {
    pub fn to_report(&self) -> Report {
        Report {
            flow_key: FlowKey {
                saddr: self.flow.saddr,
                daddr: self.flow.daddr,
                sport: self.flow.sport,
                dport: self.flow.dport,
            },
            packets_in_flight: self.flow_stats.packets_in_flight,
            bytes_in_flight: self.flow_stats.bytes_in_flight,
            bytes_pending: self.flow_stats.bytes_pending,
            rtt_sample_us: self.flow_stats.rtt_sample_us,
            was_timeout: self.flow_stats.was_timeout != 0,
            bytes_acked: self.ack_stats.bytes_acked,
            packets_acked: self.ack_stats.packets_acked,
            bytes_misordered: self.ack_stats.bytes_misordered,
            packets_misordered: self.ack_stats.packets_misordered,
            ecn_bytes: self.ack_stats.ecn_bytes,
            ecn_packets: self.ack_stats.ecn_packets,
            lost_pkts_sample: self.ack_stats.lost_pkts_sample,
            rate_incoming: self.rates.rate_incoming,
            rate_outgoing: self.rates.rate_outgoing,
            snd_cwnd: self.snd_cwnd,
            snd_ssthresh: self.snd_ssthresh,
            pacing_rate: self.pacing_rate,
            ca_state: self.ca_state,
            now: self.ack_stats.now,
        }
    }
}

#[repr(C)]
pub struct UserUpdate {
    pub cwnd_bytes: u32,
    pub pacing_rate: u64,
    pub ssthresh: u32,
    pub use_pacing: u8,   // Whether to use pacing rate when updating
    pub use_cwnd: u8,     // Whether to use cwnd when updating
    pub use_ssthresh: u8, // Whether to override ssthresh
    pub _pad: u8,
    pub flow_command: u32,
}

pub enum DatapathEvent {
    FlowCreated {
        flow_id: u64,
        init_cwnd: u32,
        mss: u32,
    },
    FlowClosed {
        flow_id: u64,
    },
    Measurement {
        flow_id: u64,
        measurement: Measurement,
    },
}

pub struct EbpfDatapath {
    obj: Object,
    _link: Link, // Keep link alive
    events: Arc<Mutex<VecDeque<DatapathEvent>>>,
}

impl EbpfDatapath {
    pub fn new(ebpf_path: &str, struct_ops_name: &str) -> Result<Self> {
        info!("Loading eBPF object file: {}", ebpf_path);
        let obj_path = Path::new(ebpf_path);
        if !obj_path.exists() {
            anyhow::bail!("eBPF object file not found: {:?}.", obj_path);
        }

        let mut builder = ObjectBuilder::default();
        builder.debug(true);

        let open_obj = builder
            .open_file(obj_path)
            .context("Failed to open BPF object file")?;

        let mut obj = open_obj
            .load()
            .context("Failed to load BPF object into kernel")?;
        info!("BPF object loaded successfully, attempting to register with TCP");

        // Attach to tcp stack via struct_ops
        let mut struct_ops_map = obj
            .maps_mut()
            .find(|m| m.name() == struct_ops_name)
            .with_context(|| format!("Failed to find '{}' struct_ops map", struct_ops_name))?;
        let link = struct_ops_map
            .attach_struct_ops()
            .context("Failed to attach struct_ops to TCP stack")?;

        info!(
            "struct_ops attached - '{}' registered with TCP",
            struct_ops_name
        );

        // Set up ring buffers
        let events = Arc::new(Mutex::new(VecDeque::new()));

        // Get measurements ring buffer map
        let measurements_map = obj
            .maps()
            .find(|m| m.name() == "measurements")
            .context("Failed to find 'measurements' map")?;

        let events_clone = events.clone();
        let mut rb_builder = RingBufferBuilder::new();

        rb_builder
            .add(&measurements_map, move |data: &[u8]| { 
                let m = unsafe { &*(data.as_ptr() as *const Measurement) };

                let flow = m.flow;
                let flow_id = flow_key_to_id(&flow);
                let rate_incoming = m.rates.rate_incoming;
                let rate_outgoing = m.rates.rate_outgoing;
                let bytes_acked = m.ack_stats.bytes_acked;
                let lost_pkts_sample = m.ack_stats.lost_pkts_sample;
                let rtt_sample_us = m.flow_stats.rtt_sample_us;
                let bytes_in_flight = m.flow_stats.bytes_in_flight;
                let measurement_type = m.measurement_type;

                debug!(
                    "Measurement: flow={:016x}, rate_incoming={}, rate_outgoing={}, acked={}, loss={}, rtt={}us, inflight={}, type={}",
                    rate_incoming,
                    rate_outgoing,
                    flow_id,
                    bytes_acked,
                    lost_pkts_sample,
                    rtt_sample_us,
                    bytes_in_flight,
                    measurement_type
                );

                events_clone
                    .lock()
                    .unwrap()
                    .push_back(DatapathEvent::Measurement {
                        flow_id,
                        measurement: *m,
                    });
                0
            })
            .context("Failed to add measurements ring buffer")?;

        // Get flow events ring buffer map
        let flow_events_map = obj
            .maps()
            .find(|m| m.name() == "flow_events")
            .context("Failed to find 'flow_events' map")?;

        let events_clone2 = events.clone();
        rb_builder
            .add(&flow_events_map, move |data: &[u8]| {
                let e = unsafe { &*(data.as_ptr() as *const FlowEvent) };
                let flow_id = flow_key_to_id(&e.flow);

                let event = match e.event_type {
                    1 => {
                        debug!("Flow created: {:016x}", flow_id);
                        DatapathEvent::FlowCreated {
                            flow_id,
                            init_cwnd: e.init_cwnd,
                            mss: e.mss,
                        }
                    }
                    2 => {
                        debug!("Flow closed: {:016x}", flow_id);
                        DatapathEvent::FlowClosed { flow_id }
                    }
                    _ => {
                        warn!("Unknown flow event type: {}", e.event_type);
                        return 0;
                    }
                };

                events_clone2.lock().unwrap().push_back(event);
                0
            })
            .context("Failed to add flow_events ring buffer")?;

        let rb = rb_builder.build().context("Failed to build ring buffers")?;
        std::thread::spawn(move || {
            loop {
                if let Err(e) = rb.poll(std::time::Duration::from_millis(100)) {
                    warn!("Ring buffer poll error: {}", e);
                }
            }
        });

        Ok(Self {
            obj,
            _link: link,
            events,
        })
    }

    pub fn poll(&mut self, _timeout_ms: u64) -> Result<Vec<DatapathEvent>> {
        let mut events = self.events.lock().unwrap();
        let result: Vec<_> = events.drain(..).collect();
        Ok(result)
    }

    pub fn cleanup_flow(&self, flow_id: u64) {
        cleanup_flow_key(flow_id);
    }

    pub fn update_cwnd(&mut self, flow_id: u64, cwnd_bytes: u32) -> Result<()> {
        let update = UserUpdate {
            cwnd_bytes,
            pacing_rate: 0,
            ssthresh: 0,
            use_pacing: 0,
            use_cwnd: 1,
            use_ssthresh: 0,
            _pad: 0,
            flow_command: 0,
        };
        self.send_user_update(flow_id, &update)
    }

    pub fn update_pacing_rate(&mut self, flow_id: u64, pacing_rate: u64) -> Result<()> {
        let update = UserUpdate {
            cwnd_bytes: 0,
            pacing_rate,
            ssthresh: 0,
            use_pacing: 1,
            use_cwnd: 0,
            use_ssthresh: 0,
            _pad: 0,
            flow_command: 0,
        };
        self.send_user_update(flow_id, &update)
    }

    pub fn update_cwnd_and_pacing(
        &mut self,
        flow_id: u64,
        cwnd_bytes: u32,
        pacing_rate: u64,
    ) -> Result<()> {
        let update = UserUpdate {
            cwnd_bytes,
            pacing_rate,
            ssthresh: 0,
            use_pacing: 1,
            use_cwnd: 1,
            use_ssthresh: 0,
            _pad: 0,
            flow_command: 0,
        };
        self.send_user_update(flow_id, &update)
    }

    pub fn send_user_update(&mut self, flow_id: u64, update: &UserUpdate) -> Result<()> {
        let map = self
            .obj
            .maps()
            .find(|m| m.name() == "user_command_map")
            .context("Failed to find user_command_map")?;

        let key = id_to_flow_key(flow_id);

        let key_bytes = unsafe {
            std::slice::from_raw_parts(
                &key as *const _ as *const u8,
                std::mem::size_of::<FlowKey>(),
            )
        };

        let value_bytes = unsafe {
            std::slice::from_raw_parts(
                update as *const _ as *const u8,
                std::mem::size_of::<UserUpdate>(),
            )
        };

        map.update(key_bytes, value_bytes, MapFlags::ANY)
            .context("Failed to update user_command_map")?;

        Ok(())
    }
}

// Mapping from flow_id to flow_key for map lookups
lazy_static! {
    static ref FLOW_KEY_MAP: Mutex<HashMap<u64, FlowKey>> = Mutex::new(HashMap::new());
}

fn flow_key_to_id(key: &FlowKey) -> u64 {
    let id = ((key.saddr as u64) << 32) | (key.daddr as u64);
    FLOW_KEY_MAP.lock().unwrap().insert(id, *key);
    id
}

fn id_to_flow_key(id: u64) -> FlowKey {
    FLOW_KEY_MAP
        .lock()
        .unwrap()
        .get(&id)
        .copied()
        .unwrap_or_else(|| {
            warn!("Flow key not found for id {:016x}, using partial key", id);
            FlowKey {
                saddr: (id >> 32) as u32,
                daddr: id as u32,
                sport: 0,
                dport: 0,
            }
        })
}

fn cleanup_flow_key(id: u64) {
    FLOW_KEY_MAP.lock().unwrap().remove(&id);
}
