//! BBR congestion control algorithm implementation
//!
//! Model of the network path:
//! ```no-run
//!    bottleneck_bandwidth = windowed_max(delivered / elapsed, 10 round trips)
//!    min_rtt = windowed_min(rtt, 10 seconds)
//! ```
//! ```no-run
//! pacing_rate = pacing_gain * bottleneck_bandwidth
//! cwnd = max(cwnd_gain * bottleneck_bandwidth * min_rtt, 4)
//! ```
//!
//! This implementation covers `PROBE_BW` and `PROBE_RTT`. STARTUP/DRAIN modes
//! and finer points such as policing detection are left as future work.

use trafik_runtime::{GenericAlgorithm, GenericFlow, Report};
use std::time::{Duration, Instant};
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProbeBwPhase {
    Drain,  // 0.75x gain, 1 RTT
    Cruise, // 1.0x gain, 2 RTTs
    Probe,  // 1.25x gain, 8 RTTs
}

#[derive(Clone, Copy, PartialEq)]
enum BbrMode {
    ProbeBw(ProbeBwPhase),
    ProbeRtt,
}

pub const PROBE_RTT_INTERVAL_SECONDS: i64 = 10;
const PROBE_RTT_DURATION_MS: u64 = 200;
const MIN_CWND_PACKETS: u32 = 4;

pub struct Bbr {
    mss: u32,
    init_cwnd: u32,
    cwnd: u32,
    pacing_rate: u64,
    mode: BbrMode,
    probe_rtt_interval: Duration,
    bottleneck_bw: f64,
    bottleneck_bw_timeout: Instant,
    min_rtt_us: u32,
    min_rtt_timeout: Instant,
    probe_bw_timer: Instant,
    probe_rtt_done_timestamp: Option<Instant>,
    probe_rtt_inflight_reached: bool,
    start: Instant,
}

impl Bbr {
    fn new(init_cwnd: u32, mss: u32) -> Self {
        let now = Instant::now();
        let probe_rtt_interval = Duration::from_secs(PROBE_RTT_INTERVAL_SECONDS as u64);
        let initial_bw = 125_000.0; // 1 Mbps in bytes/sec

        Self {
            mss,
            init_cwnd,
            cwnd: init_cwnd,
            pacing_rate: initial_bw as u64,
            mode: BbrMode::ProbeBw(ProbeBwPhase::Drain),
            probe_rtt_interval,
            bottleneck_bw: initial_bw,
            bottleneck_bw_timeout: now + probe_rtt_interval,
            min_rtt_us: 1_000_000,
            min_rtt_timeout: now + probe_rtt_interval,
            probe_bw_timer: now,
            probe_rtt_done_timestamp: None,
            probe_rtt_inflight_reached: false,
            start: now,
        }
    }

    fn pacing_gain(&self) -> f64 {
        match self.mode {
            BbrMode::ProbeBw(ProbeBwPhase::Drain) => 0.75,
            BbrMode::ProbeBw(ProbeBwPhase::Cruise) => 1.0,
            BbrMode::ProbeBw(ProbeBwPhase::Probe) => 1.25,
            BbrMode::ProbeRtt => 0.75,
        }
    }

    fn cwnd_gain(&self) -> f64 {
        match self.mode {
            BbrMode::ProbeBw(ProbeBwPhase::Drain) => 0.75,
            BbrMode::ProbeBw(ProbeBwPhase::Cruise) => 1.0,
            BbrMode::ProbeBw(ProbeBwPhase::Probe) => 1.25,
            BbrMode::ProbeRtt => 0.5,
        }
    }

    fn calculate_pacing_rate(&self) -> u64 {
        (self.bottleneck_bw * self.pacing_gain()) as u64
    }

    fn calculate_target_cwnd(&self) -> u32 {
        let bdp = self.bottleneck_bw * (f64::from(self.min_rtt_us) / 1_000_000.0);
        let target = (bdp * self.cwnd_gain() * 2.0) as u32;
        target.max(MIN_CWND_PACKETS * self.mss)
    }

    fn update_bandwidth(&mut self, report: &Report) {
        if report.bytes_acked == 0 {
            return;
        }
        let rtt_sec = f64::from(report.rtt_sample_us) / 1_000_000.0;
        if rtt_sec > 0.0 {
            let throughput = f64::from(report.bytes_acked) / rtt_sec;
            if throughput > self.bottleneck_bw {
                self.bottleneck_bw = throughput;
                self.bottleneck_bw_timeout = Instant::now() + self.probe_rtt_interval;
                debug!(
                    "Updated bottleneck_bw to {:.2} Mbps",
                    self.bottleneck_bw / 125_000.0
                );
            }
        }
    }

    fn update_min_rtt(&mut self, rtt_us: u32) {
        if rtt_us < self.min_rtt_us {
            self.min_rtt_us = rtt_us;
            self.min_rtt_timeout = Instant::now() + self.probe_rtt_interval;
            debug!("Updated min_rtt to {} us", self.min_rtt_us);
        }
    }

    fn handle_probe_bw(&mut self, report: &Report) {
        let now = Instant::now();
        let min_rtt = Duration::from_micros(self.min_rtt_us as u64);
        let time_in_state = now.duration_since(self.probe_bw_timer);

        if let BbrMode::ProbeBw(phase) = self.mode {
            let (threshold, next_phase) = match phase {
                ProbeBwPhase::Drain => (min_rtt, ProbeBwPhase::Cruise),
                ProbeBwPhase::Cruise => (min_rtt * 2, ProbeBwPhase::Probe),
                ProbeBwPhase::Probe => (min_rtt * 8, ProbeBwPhase::Drain),
            };
            if time_in_state >= threshold {
                self.mode = BbrMode::ProbeBw(next_phase);
                self.probe_bw_timer = now;
                debug!("PROBE_BW: {:?} -> {:?}", phase, next_phase);
            }
        }

        if now > self.min_rtt_timeout {
            self.mode = BbrMode::ProbeRtt;
            self.min_rtt_us = 0x3fff_ffff;
            self.probe_rtt_done_timestamp = None;
            self.probe_rtt_inflight_reached = false;
            info!(min_rtt_us = report.rtt_sample_us, "Entering PROBE_RTT mode");
        }
    }

    fn handle_probe_rtt(&mut self, report: &Report) {
        let now = Instant::now();

        if !self.probe_rtt_inflight_reached && report.packets_in_flight <= MIN_CWND_PACKETS {
            self.probe_rtt_inflight_reached = true;
            self.probe_rtt_done_timestamp = Some(now);
            debug!("PROBE_RTT: reached target inflight");
        }

        if let Some(done_time) = self.probe_rtt_done_timestamp
            && now.duration_since(done_time) >= Duration::from_millis(PROBE_RTT_DURATION_MS) {
                self.mode = BbrMode::ProbeBw(ProbeBwPhase::Drain);
                self.probe_bw_timer = now;
                self.min_rtt_timeout = now + self.probe_rtt_interval;
                info!(min_rtt_us = self.min_rtt_us, "Exiting PROBE_RTT mode");
            }
    }
}

impl GenericFlow for Bbr {
    fn curr_cwnd(&self) -> u32 {
        self.cwnd
    }

    fn set_cwnd(&mut self, cwnd: u32) {
        self.cwnd = cwnd;
    }

    fn curr_pacing_rate(&self) -> Option<u64> {
        Some(self.pacing_rate)
    }

    fn increase(&mut self, report: &Report) {
        self.update_bandwidth(report);
        self.update_min_rtt(report.rtt_sample_us);

        match self.mode {
            BbrMode::ProbeBw(_) => self.handle_probe_bw(report),
            BbrMode::ProbeRtt => self.handle_probe_rtt(report),
        }

        let old_cwnd = self.cwnd;
        let old_pacing_rate = self.pacing_rate;
        self.cwnd = self.calculate_target_cwnd();
        self.pacing_rate = self.calculate_pacing_rate();

        if old_cwnd != self.cwnd || old_pacing_rate != self.pacing_rate {
            debug!(
                mode = match self.mode {
                    BbrMode::ProbeBw(_) => "PROBE_BW",
                    BbrMode::ProbeRtt => "PROBE_RTT",
                },
                old_cwnd = old_cwnd,
                new_cwnd = self.cwnd,
                old_pacing_Mbps = old_pacing_rate as f64 / 125_000.0,
                new_pacing_Mbps = self.pacing_rate as f64 / 125_000.0,
                bottleneck_bw_Mbps = self.bottleneck_bw / 125_000.0,
                min_rtt_us = self.min_rtt_us,
                "BBR update"
            );
        }
    }

    fn reduction(&mut self, _report: &Report) {
        self.bottleneck_bw *= 0.95;
        self.pacing_rate = self.calculate_pacing_rate();
        debug!(
            "Loss detected, reduced bottleneck_bw to {:.2} Mbps, pacing to {:.2} Mbps",
            self.bottleneck_bw / 125_000.0,
            self.pacing_rate as f64 / 125_000.0
        );
    }

    fn reset(&mut self) {
        let now = Instant::now();
        self.cwnd = self.init_cwnd;
        self.mode = BbrMode::ProbeBw(ProbeBwPhase::Drain);
        self.bottleneck_bw = 125_000.0;
        self.pacing_rate = 125_000;
        self.bottleneck_bw_timeout = now + self.probe_rtt_interval;
        self.min_rtt_us = 1_000_000;
        self.min_rtt_timeout = now + self.probe_rtt_interval;
        self.probe_bw_timer = now;
        self.probe_rtt_done_timestamp = None;
        self.probe_rtt_inflight_reached = false;
        info!("BBR flow reset");
    }
}

pub struct BbrAlgorithm;

impl GenericAlgorithm for BbrAlgorithm {
    fn name(&self) -> &str {
        "bbr"
    }

    fn create_flow(&self, init_cwnd: u32, mss: u32) -> Box<dyn GenericFlow> {
        Box::new(Bbr::new(init_cwnd, mss))
    }
}
