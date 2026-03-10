use ebpf_ccp_generic::{GenericAlgorithm, GenericFlow, Report};

#[derive(Default)]
pub struct Reno {
    mss: u32,
    init_cwnd: f64,
    cwnd: f64,
}

impl GenericFlow for Reno {
    fn curr_cwnd(&self) -> u32 {
        self.cwnd as u32
    }

    fn set_cwnd(&mut self, cwnd: u32) {
        self.cwnd = f64::from(cwnd);
    }

    fn increase(&mut self, report: &Report) {
        self.cwnd += f64::from(self.mss) * (f64::from(report.bytes_acked) / self.cwnd);
    }

    fn reduction(&mut self, _report: &Report) {
        self.cwnd /= 2.0;
        if self.cwnd <= self.init_cwnd {
            self.cwnd = self.init_cwnd;
        }
    }

    fn reset(&mut self) {
        self.cwnd = self.init_cwnd;
    }
}

pub struct RenoAlgorithm;

impl GenericAlgorithm for RenoAlgorithm {
    fn name(&self) -> &str {
        "reno"
    }

    fn create_flow(&self, init_cwnd: u32, mss: u32) -> Box<dyn GenericFlow> {
        Box::new(Reno {
            mss,
            init_cwnd: f64::from(init_cwnd),
            cwnd: f64::from(init_cwnd),
        })
    }
}
