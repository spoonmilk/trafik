//! CUBIC congestion control algorithm implementation

use trafik_runtime::{GenericAlgorithm, GenericFlow, Report};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Cubic {
    pkt_size: u32,
    init_cwnd: u32,

    cwnd: f64,
    cwnd_cnt: f64,
    tcp_friendliness: bool,
    beta: f64,
    fast_convergence: bool,
    c: f64,
    wlast_max: f64,
    epoch_start: Option<Instant>,
    origin_point: f64,
    d_min: Option<Duration>,
    wtcp: f64,
    k: f64,
    ack_cnt: f64,
    cnt: f64,
}

impl Cubic {
    fn cubic_update(&mut self) {
        self.ack_cnt += 1.0;
        if self.epoch_start.is_none() {
            self.epoch_start = Some(Instant::now());
            if self.cwnd < self.wlast_max {
                let temp = (self.wlast_max - self.cwnd) / self.c;
                self.k = (temp.max(0.0)).powf(1.0 / 3.0);
                self.origin_point = self.wlast_max;
            } else {
                self.k = 0.0;
                self.origin_point = self.cwnd;
            }

            self.ack_cnt = 1.0;
            self.wtcp = self.cwnd
        }

        let t = Instant::now()
            .duration_since(self.epoch_start.unwrap())
            .as_secs_f64();
        let target = self.origin_point + self.c * ((t - self.k) * (t - self.k) * (t - self.k));
        if target > self.cwnd {
            self.cnt = self.cwnd / (target - self.cwnd);
        } else {
            self.cnt = 100.0 * self.cwnd;
        }

        if self.tcp_friendliness {
            self.cubic_tcp_friendliness();
        }
    }

    fn cubic_tcp_friendliness(&mut self) {
        self.wtcp += ((3.0 * self.beta) / (2.0 - self.beta)) * (self.ack_cnt / self.cwnd);
        self.ack_cnt = 0.0;
        if self.wtcp > self.cwnd {
            let max_cnt = self.cwnd / (self.wtcp - self.cwnd);
            if self.cnt > max_cnt {
                self.cnt = max_cnt;
            }
        }
    }

    fn cubic_reset(&mut self) {
        self.wlast_max = 0.0;
        self.epoch_start = None;
        self.origin_point = 0.0;
        self.d_min = None;
        self.wtcp = 0.0;
        self.k = 0.0;
        self.ack_cnt = 0.0;
    }
}

impl GenericFlow for Cubic {
    fn curr_cwnd(&self) -> u32 {
        (self.cwnd * f64::from(self.pkt_size)) as u32
    }

    fn set_cwnd(&mut self, cwnd: u32) {
        self.cwnd = f64::from(cwnd) / f64::from(self.pkt_size);
    }

    fn increase(&mut self, report: &Report) {
        let f_rtt = Duration::from_micros(report.rtt_sample_us as _);
        let no_of_acks = ((f64::from(report.bytes_acked)) / (f64::from(self.pkt_size))) as u32;
        for _ in 0..no_of_acks {
            match self.d_min {
                None => self.d_min = Some(f_rtt),
                Some(dmin) if f_rtt < dmin => {
                    self.d_min = Some(f_rtt);
                }
                _ => (),
            }

            self.cubic_update();
            if self.cwnd_cnt > self.cnt {
                self.cwnd += 1.0;
                self.cwnd_cnt = 0.0;
            } else {
                self.cwnd_cnt += 1.0;
            }
        }
    }

    fn reduction(&mut self, _report: &Report) {
        self.epoch_start = None;
        if self.cwnd < self.wlast_max && self.fast_convergence {
            self.wlast_max = self.cwnd * ((2.0 - self.beta) / 2.0);
        } else {
            self.wlast_max = self.cwnd;
        }

        self.cwnd *= 1.0 - self.beta;
        if self.cwnd as u32 <= self.init_cwnd {
            self.cwnd = f64::from(self.init_cwnd);
        }
    }

    fn reset(&mut self) {
        self.cubic_reset();
    }
}

pub struct CubicAlgorithm;

impl GenericAlgorithm for CubicAlgorithm {
    fn name(&self) -> &str {
        "cubic"
    }

    fn create_flow(&self, init_cwnd: u32, mss: u32) -> Box<dyn GenericFlow> {
        let init_cwnd_pkts = init_cwnd / mss;
        Box::new(Cubic {
            pkt_size: mss,
            init_cwnd: init_cwnd_pkts,
            cwnd: f64::from(init_cwnd_pkts),
            cwnd_cnt: 0.0,
            tcp_friendliness: true,
            beta: 0.7,
            fast_convergence: true,
            c: 0.4,
            wlast_max: 0.0,
            epoch_start: None,
            origin_point: 0.0,
            d_min: None,
            wtcp: 0.0,
            k: 0.0,
            ack_cnt: 0.0,
            cnt: 0.0,
        })
    }
}
