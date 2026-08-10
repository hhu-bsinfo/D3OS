//! Windowed streaming bandwidth benchmark. Ported from `rust-rdma-bench/src/bench/bandwidth.rs`;
//! RC and UC run through the exact same code here too — UC's loss just surfaces via the
//! receiver's idle-timeout early exit and a "never arrived" count instead of always draining to
//! `iterations`.

use crate::bench::{self, Role, IDLE_TIMEOUT_US};
use crate::comm::Conn;
use crate::error::Result;
use alloc::vec;
use ibverbs::{ibv_wc, CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair};
use rdma::ibv_send_flags;
use time::get_time_in_us;

pub fn run(
    pd: &ProtectionDomain,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    role: Role,
    msg_size: usize,
    iterations: usize,
    tx_depth: usize,
) -> Result<()> {
    let mut mr = pd.allocate::<u8>(msg_size)?;

    match role {
        Role::Client => send(&mut mr, cq, qp, conn, msg_size, iterations, tx_depth),
        Role::Server => receive(&mut mr, cq, qp, conn, msg_size, iterations, tx_depth),
    }
}

fn send(
    mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
    tx_depth: usize,
) -> Result<()> {
    conn.sync()?; // wait for "ready"
    let t0 = get_time_in_us();

    let window = tx_depth.min(iterations);
    for i in 0..window {
        unsafe { qp.post_send(mr, vec![vec![0..msg_size]], vec![i as u64], vec![ibv_send_flags::SIGNALED])? };
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![ibv_wc::default(); tx_depth.max(1)];

    while completed < iterations {
        let completions = cq.poll(&mut wc)?;
        let n = completions.len();
        for c in completions.iter() {
            bench::completion_error(c)?;
        }
        completed += n;
        for _ in 0..n {
            if posted < iterations {
                unsafe { qp.post_send(mr, vec![vec![0..msg_size]], vec![posted as u64], vec![ibv_send_flags::SIGNALED])? };
                posted += 1;
            }
        }
    }

    let elapsed_us = get_time_in_us() - t0;
    conn.sync()?;

    report(msg_size, iterations, tx_depth, elapsed_us);
    Ok(())
}

fn receive(
    mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
    tx_depth: usize,
) -> Result<()> {
    let window = tx_depth.min(iterations);
    for i in 0..window {
        unsafe { qp.post_receive(mr, vec![vec![0..msg_size]], vec![i as u64])? };
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![ibv_wc::default(); tx_depth.max(1)];

    conn.sync()?; // "ready"
    let mut last_progress = get_time_in_us();
    while completed < iterations {
        let completions = cq.poll(&mut wc)?;
        let n = completions.len();
        for c in completions.iter() {
            bench::completion_error(c)?;
        }

        if n == 0 {
            if get_time_in_us() - last_progress >= IDLE_TIMEOUT_US {
                break;
            }
            continue;
        }

        last_progress = get_time_in_us();
        completed += n;
        for _ in 0..n {
            if posted < iterations {
                unsafe { qp.post_receive(mr, vec![vec![0..msg_size]], vec![posted as u64])? };
                posted += 1;
            }
        }
    }
    conn.sync()?; // "done draining"

    if completed < iterations {
        terminal::println!("received {} of {} messages ({} never arrived)", completed, iterations, iterations - completed);
    } else {
        terminal::println!("received {} messages", completed);
    }
    Ok(())
}

fn report(msg_size: usize, iterations: usize, tx_depth: usize, elapsed_us: usize) {
    let secs = elapsed_us as f64 / 1_000_000.0;
    let bytes = iterations as f64 * msg_size as f64;
    let bw_gbps = bytes * 8.0 / secs / 1e9;
    let msg_rate_mpps = iterations as f64 / secs / 1e6;
    terminal::println!("{:>8}  {:>12}  {:>10}  {:>18}  {:>14}", "#bytes", "#iterations", "tx_depth", "BW avg[Gb/sec]", "MsgRate[Mpps]");
    terminal::println!("{:>8}  {:>12}  {:>10}  {:>18.2}  {:>14.6}", msg_size, iterations, tx_depth, bw_gbps, msg_rate_mpps);
}
