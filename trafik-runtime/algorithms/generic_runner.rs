use std::collections::HashMap;

use crate::bpf::DatapathEvent;
use anyhow::anyhow;
use trafik_runtime::GenericAlgorithm;
use tracing::{debug, info, warn};

use crate::algorithms::{AlgorithmRunner, CwndUpdate};

struct FlowState {
    flow: Box<dyn trafik_runtime::GenericFlow>,
    last_lost_pkts: u32,
    last_cwnd: u32,
}

pub struct GenericRunner<A: GenericAlgorithm> {
    algorithm: A,
    flows: HashMap<u64, FlowState>,
    ebpf_path: &'static str,
    struct_ops_name: &'static str,
}

impl<A: GenericAlgorithm> GenericRunner<A> {
    pub fn new(algorithm: A, ebpf_path: &'static str, struct_ops_name: &'static str) -> Self {
        Self {
            algorithm,
            flows: HashMap::new(),
            ebpf_path,
            struct_ops_name,
        }
    }
}

impl<A: GenericAlgorithm> AlgorithmRunner for GenericRunner<A> {
    fn name(&self) -> &str {
        self.algorithm.name()
    }

    fn ebpf_path(&self) -> &str {
        self.ebpf_path
    }

    fn struct_ops_name(&self) -> &str {
        self.struct_ops_name
    }

    fn handle_event(&mut self, event: DatapathEvent) -> anyhow::Result<Option<CwndUpdate>> {
        match event {
            DatapathEvent::FlowCreated {
                flow_id,
                init_cwnd,
                mss,
            } => {
                info!(
                    "Flow created: {:016x}, init_cwnd={} bytes, mss={} bytes",
                    flow_id, init_cwnd, mss
                );
                let flow_state = FlowState {
                    flow: self.algorithm.create_flow(init_cwnd, mss),
                    last_lost_pkts: 0,
                    last_cwnd: init_cwnd,
                };
                self.flows.insert(flow_id, flow_state);
                Ok(None)
            }

            DatapathEvent::Measurement {
                flow_id,
                measurement,
            } => {
                let flow_state = self
                    .flows
                    .get_mut(&flow_id)
                    .ok_or_else(|| anyhow!("Unknown flow {}", flow_id))?;

                let report = measurement.to_report();

                // lost_pkts_sample is cumulative (tp->lost_out), not incremental
                let new_loss = report.lost_pkts_sample > flow_state.last_lost_pkts;
                let old_cwnd = flow_state.flow.curr_cwnd();

                if report.was_timeout {
                    warn!(
                        "Flow {:016x}: timeout detected, resetting (cwnd: {} bytes)",
                        flow_id, old_cwnd
                    );
                    flow_state.flow.reset();
                    flow_state.last_lost_pkts = 0;
                } else if new_loss {
                    flow_state.flow.reduction(&report);
                    flow_state.last_lost_pkts = report.lost_pkts_sample;
                } else if report.bytes_acked > 0 {
                    flow_state.flow.increase(&report);
                    if report.lost_pkts_sample == 0 {
                        flow_state.last_lost_pkts = 0;
                    }
                }

                let new_cwnd = flow_state.flow.curr_cwnd();
                let pacing_rate = flow_state.flow.curr_pacing_rate();

                if old_cwnd != new_cwnd {
                    debug!(
                        "Flow {:016x}: cwnd {} -> {} bytes (acked={}, rtt={}us, inflight={})",
                        flow_id,
                        old_cwnd,
                        new_cwnd,
                        report.bytes_acked,
                        report.rtt_sample_us,
                        report.bytes_in_flight
                    );
                }

                if new_cwnd != flow_state.last_cwnd || pacing_rate.is_some() {
                    flow_state.last_cwnd = new_cwnd;
                    Ok(Some(CwndUpdate {
                        flow_id,
                        cwnd_bytes: new_cwnd,
                        pacing_rate,
                    }))
                } else {
                    Ok(None)
                }
            }

            DatapathEvent::FlowClosed { flow_id } => {
                info!("Flow closed: {:016x}", flow_id);
                self.flows.remove(&flow_id);
                Ok(None)
            }
        }
    }

    fn cleanup(&mut self) {
        info!("Cleaning up {} datapath flows", self.flows.len());
        self.flows.clear();
    }
}
