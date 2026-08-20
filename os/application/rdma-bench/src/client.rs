//! The client side: runs the benchmark matrix the CLI asked for — one run per (mode, size) pair.
//! With neither `--mode` nor `--size` given that matrix is the complete suite (every mode over a
//! power-of-two size sweep); with both given it collapses to the single run the client has always
//! done.
//!
//! Every pair is an ordinary run on the wire: a fresh TCP connection, a fresh completion queue
//! and a fresh queue pair, using the unchanged handshake protocol. That keeps the server (and a
//! native `rust-rdma-bench` peer) oblivious to suites — it only has to be serving in a loop, i.e.
//! started with `--listen`.

use crate::bench::{self, Role};
use crate::cli::{ClientArgs, Mode, Transport};
use crate::comm::{self, BenchmarkRequest, ClientEndpoint, HandshakeAck};
use crate::device;
use crate::error::{other, Result};
use crate::report::{self, Report};
use crate::transport;
use alloc::vec::Vec;
use concurrent::thread::sleep;
use core::net::IpAddr;
use ibverbs::{Context, ProtectionDomain};
use terminal::println;

/// Pause between runs, giving the server time to tear the finished connection down and get back
/// into `accept()` before the next one arrives.
const SETTLE_MS: usize = 250;

/// Everything one benchmark run needs.
struct RunParams {
    host: IpAddr,
    port: u16,
    transport: Transport,
    mode: Mode,
    size: usize,
    iterations: usize,
    tx_depth: usize,
}

pub fn run(args: ClientArgs) -> Result<()> {
    let ctx = device::open()?;
    let pd = ctx.alloc_pd()?;

    if args.is_single_run() {
        let params = RunParams {
            host: args.host,
            port: args.port,
            transport: args.transport,
            mode: args.modes[0],
            size: args.sizes[0],
            iterations: args.iterations,
            tx_depth: args.tx_depth,
        };
        let report = run_once(&ctx, &pd, &params, true)?;
        report.print(params.mode);
        return Ok(());
    }

    run_suite(&ctx, &pd, &args)
}

/// Runs every (mode, size) pair, printing one table per mode with a row per size. A failing run
/// is reported in place and does not abort the rest of the sweep.
fn run_suite(ctx: &Context, pd: &ProtectionDomain, args: &ClientArgs) -> Result<()> {
    println!(
        "running {} mode(s) over {} message size(s), transport={:?}, iterations={}, tx_depth={}",
        args.modes.len(),
        args.sizes.len(),
        args.transport,
        args.iterations,
        args.tx_depth
    );
    println!("(the peer must be running as `rdma-bench server --listen`)");

    let mut failures: Vec<(Mode, usize)> = Vec::new();
    let mut first_run = true;

    for &mode in &args.modes {
        let sizes: Vec<usize> = args.sizes.iter().copied().filter(|&s| s >= mode.min_msg_size()).collect();
        let skipped = args.sizes.len() - sizes.len();

        println!("");
        println!("=== {} ===", mode.name());
        if skipped > 0 {
            println!("(skipping {} size(s) below {} bytes, the minimum for this mode)", skipped, mode.min_msg_size());
        }
        println!("{}", report::header(mode));

        for size in sizes {
            // Every run after the first reconnects to a server that just finished one.
            if !first_run {
                sleep(SETTLE_MS);
            }
            first_run = false;

            let params = RunParams {
                host: args.host,
                port: args.port,
                transport: args.transport,
                mode,
                size,
                iterations: args.iterations,
                tx_depth: args.tx_depth,
            };

            match run_once(ctx, pd, &params, false) {
                Ok(result) => {
                    match result.row() {
                        Some(row) => println!("{}", row),
                        None => println!("{:>8}  (no result)", size),
                    }
                    if let Some(notes) = result.notes() {
                        println!("{:>8}  {}", "", notes);
                    }
                }
                Err(e) => {
                    println!("{:>8}  failed: {:?}", size, e);
                    failures.push((mode, size));
                }
            }
        }
    }

    println!("");
    if failures.is_empty() {
        println!("all runs succeeded");
    } else {
        println!("{} run(s) failed:", failures.len());
        for (mode, size) in &failures {
            println!("  {} @ {} bytes", mode.name(), size);
        }
    }
    Ok(())
}

/// Connects, handshakes and runs a single benchmark, leaving no RDMA or TCP resources behind, so
/// the caller can invoke it repeatedly against a `--listen` server.
///
/// `verbose` gates the per-run chatter a sweep would otherwise repeat for every size.
fn run_once(ctx: &Context, pd: &ProtectionDomain, params: &RunParams, verbose: bool) -> Result<Report> {
    let cq = ctx.create_cq((2 * params.tx_depth) as i32, 0)?;

    let prepared = transport::build(params.transport, params.mode, pd, &cq, params.tx_depth)?;
    let local_endpoint = prepared.endpoint();

    let conn = comm::connect(params.host, params.port)?;
    conn.send_msg(&BenchmarkRequest {
        transport: params.transport,
        mode: params.mode,
        msg_size: params.size,
        iterations: params.iterations,
        tx_depth: params.tx_depth,
    })?;

    let ack: HandshakeAck = conn.recv_msg()?;
    let remote_endpoint = match ack {
        HandshakeAck::Unsupported(reason) => {
            println!("server rejected benchmark request: {}", reason);
            return Err(other("server rejected benchmark request"));
        }
        HandshakeAck::Ok { endpoint } => endpoint,
    };

    if verbose {
        println!("remote-endpoint = {:?}", remote_endpoint);
    }

    let mut qp = prepared.handshake(remote_endpoint)?;
    conn.send_msg(&ClientEndpoint { endpoint: local_endpoint })?;

    bench::run(
        params.mode,
        pd,
        &cq,
        &mut qp,
        &conn,
        Role::Client,
        params.size,
        params.iterations,
        params.tx_depth,
    )
}
