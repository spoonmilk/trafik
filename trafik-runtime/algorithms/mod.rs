//! Provides a runner for any algorithm

pub mod bbr;
pub mod cubic;
pub mod generic_runner;
pub mod reno;

use crate::bpf::DatapathEvent;
use anyhow::Result;

pub trait AlgorithmRunner: Send {
    fn name(&self) -> &str;
    fn ebpf_path(&self) -> &str;

    #[allow(dead_code)]
    fn struct_ops_name(&self) -> &str;

    fn handle_event(&mut self, event: DatapathEvent) -> Result<Option<CwndUpdate>>;

    fn cleanup(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct CwndUpdate {
    pub flow_id: u64,
    pub cwnd_bytes: u32,
    pub pacing_rate: Option<u64>,
}

pub struct AlgorithmRegistry;

impl AlgorithmRegistry {
    pub fn get(name: &str) -> Result<Box<dyn AlgorithmRunner>> {
        match name {
            "cubic" => Ok(Box::new(generic_runner::GenericRunner::new(
                cubic::CubicAlgorithm,
                "ebpf/.output/datapath-cubic.bpf.o",
                "ebpf_ccp_cubic",
            ))),
            "reno" => Ok(Box::new(generic_runner::GenericRunner::new(
                reno::RenoAlgorithm,
                "ebpf/.output/datapath-reno.bpf.o",
                "ebpf_ccp_reno",
            ))),
            "generic-cubic" => Ok(Box::new(generic_runner::GenericRunner::new(
                cubic::CubicAlgorithm,
                "ebpf/.output/generic.bpf.o",
                "ebpf_ccp_gen",
            ))),
            "generic-reno" => Ok(Box::new(generic_runner::GenericRunner::new(
                reno::RenoAlgorithm,
                "ebpf/.output/generic.bpf.o",
                "ebpf_ccp_gen",
            ))),
            "generic-bbr" => Ok(Box::new(generic_runner::GenericRunner::new(
                bbr::BbrAlgorithm,
                "ebpf/.output/generic.bpf.o",
                "ebpf_ccp_gen",
            ))),
            _ => anyhow::bail!("Unknown algorithm: {}", name),
        }
    }

    pub fn list() -> Vec<&'static str> {
        vec![
            "cubic",
            "reno",
            "generic-cubic",
            "generic-reno",
            "generic-bbr",
        ]
    }
}
