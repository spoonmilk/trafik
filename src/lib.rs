/// Flow key identifying a TCP connection
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub saddr: u32,
    pub daddr: u32,
    pub sport: u16,
    pub dport: u16,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub flow_key: FlowKey,

    // Flow-level statistics
    pub packets_in_flight: u32,
    pub bytes_in_flight: u32,
    pub bytes_pending: u32,
    pub rtt_sample_us: u32,
    pub was_timeout: bool,

    // ACK-level statistics
    pub bytes_acked: u32,
    pub packets_acked: u32,
    pub bytes_misordered: u32,
    pub packets_misordered: u32,
    pub ecn_bytes: u32,
    pub ecn_packets: u32,
    pub lost_pkts_sample: u32,

    // Rate statistics
    pub rate_incoming: u32,
    pub rate_outgoing: u32,

    // Kernel context
    pub snd_cwnd: u32,
    pub snd_ssthresh: u32,
    pub pacing_rate: u64,
    pub ca_state: u8,
    pub now: u64,
}

pub trait GenericAlgorithm: Send {
    fn name(&self) -> &str;
    fn create_flow(&self, init_cwnd: u32, mss: u32) -> Box<dyn GenericFlow>;
}

pub trait GenericFlow: Send {
    fn curr_cwnd(&self) -> u32;
    fn set_cwnd(&mut self, cwnd: u32);

    fn curr_pacing_rate(&self) -> Option<u64> {
        None
    }

    fn increase(&mut self, report: &Report);
    fn reduction(&mut self, report: &Report);

    fn reset(&mut self) {}
}
