mod algorithms;
mod bpf;

use algorithms::AlgorithmRegistry;
use anyhow::Result;
use bpf::EbpfDatapath;
use clap::Parser;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "ebpf-ccp")]
#[command(about = "Congestion control with eBPF datapath")]
struct Args {
    /// Congestion control algorithm to use
    #[arg(short, long, default_value = "cubic")]
    algorithm: String,

    /// Enable verbose debug logging
    #[arg(short, long)]
    verbose: bool,

    /// List available algorithms and exit
    #[arg(long)]
    list_algorithms: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --list-algorithms flag
    if args.list_algorithms {
        println!("Available congestion control algorithms:");
        for alg in AlgorithmRegistry::list() {
            println!("  - {}", alg);
        }
        return Ok(());
    }

    // Initialize logging
    let level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_max_level(level.parse::<tracing::Level>().unwrap())
        .init();

    // Get the algorithm implementation
    let mut algorithm = AlgorithmRegistry::get(&args.algorithm)?;

    info!("Starting eBPF congestion control");
    info!("  algorithm: {}", algorithm.name());
    info!("  ebpf_path: {}", algorithm.ebpf_path());

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Shutting down");
        r.store(false, Ordering::SeqCst);
    })?;

    // Load eBPF datapath
    let mut datapath = EbpfDatapath::new(algorithm.ebpf_path(), algorithm.struct_ops_name())?;
    info!("eBPF datapath loaded and attached");

    while running.load(Ordering::SeqCst) {
        let events = match datapath.poll(100) {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to poll datapath: {}", e);
                continue;
            }
        };

        for event in events {
            // Handle flow cleanup for closed flows
            if let bpf::DatapathEvent::FlowClosed { flow_id } = &event {
                datapath.cleanup_flow(*flow_id);
            }

            // Let the algorithm handle the event
            match algorithm.handle_event(event) {
                Ok(Some(update)) => {
                    // Send cwnd and/or pacing rate update back to eBPF
                    let result = match update.pacing_rate {
                        Some(pacing_rate) => datapath.update_cwnd_and_pacing(
                            update.flow_id,
                            update.cwnd_bytes,
                            pacing_rate,
                        ),
                        None => datapath.update_cwnd(update.flow_id, update.cwnd_bytes),
                    };

                    if let Err(e) = result {
                        error!("Failed to update flow {:016x}: {:?}", update.flow_id, e);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    error!("Algorithm error: {}", e);
                }
            }
        }
    }

    info!("Shutting down eBPF daemon...");
    algorithm.cleanup();
    drop(datapath);
    info!(
        "eBPF datapath detached - '{}' unregistered from TCP",
        algorithm.name()
    );
    Ok(())
}
