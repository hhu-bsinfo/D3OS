//! Windowed streaming bandwidth benchmark. Ported from `rust-rdma-bench/src/bench/bandwidth.rs`;
//! RC and UC run through the exact same code here too — UC's loss just surfaces via the
//! receiver's idle-timeout early exit and a "never arrived" count instead of always draining to
//! `iterations`.

use crate::bench::{self, Role, IDLE_TIMEOUT_US, WARMUP_SETTLE_MS};
use crate::comm::Conn;
use crate::error::Result;
use crate::report::{BandwidthStats, Report};
use alloc::vec;
use concurrent::thread::sleep;
use ibverbs::{CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair, ReceiveWorkRequest, SendFlags, SendWorkRequest, WorkCompletion};
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
    rx_depth: usize,
) -> Result<Report> {
    let mut mr = pd.allocate::<u8>(msg_size)?;

    match role {
        Role::Client => send(&mut mr, cq, qp, conn, msg_size, iterations, tx_depth),
        Role::Server => receive(&mut mr, cq, qp, conn, msg_size, iterations, rx_depth),
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
) -> Result<Report> {
    // Warm-up: see `bench::WARMUP_SETTLE_MS`'s doc comment.
    conn.sync()?; // warm-up barrier
    sleep(WARMUP_SETTLE_MS);
    conn.sync()?; // warm-up done

    conn.sync()?; // wait for "ready"
    let t0 = get_time_in_us();

    let window = tx_depth.min(iterations);
    let send_sge = [mr.slice(0..msg_size)];
    let mut send_wr = SendWorkRequest::send(0, &send_sge, SendFlags::SIGNALED);
    for i in 0..window {
        send_wr.wr_id = i as u64;
        unsafe { qp.post_send([send_wr])? };
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![WorkCompletion::default(); tx_depth.max(1)];

    while completed < iterations {
        let completions = cq.poll(&mut wc)?;
        let n = completions.len();
        for c in completions.iter() {
            bench::completion_error(c)?;
        }
        completed += n;
        for _ in 0..n {
            if posted < iterations {
                send_wr.wr_id = posted as u64;
                unsafe { qp.post_send([send_wr])? };
                posted += 1;
            }
        }
    }

    let elapsed_us = get_time_in_us() - t0;
    conn.sync()?;

    Ok(Report::Bandwidth(BandwidthStats { msg_size, iterations, tx_depth, elapsed_us }))
}

fn receive(
    mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
    rx_depth: usize,
) -> Result<Report> {
    // Warm-up: see send()'s matching comment.
    conn.sync()?; // warm-up barrier
    sleep(WARMUP_SETTLE_MS);
    conn.sync()?; // warm-up done

    let window = rx_depth.min(iterations);
    let mut recv_wr = ReceiveWorkRequest {
        wr_id: 0,
        sges: &[mr.slice(0..msg_size)],
    };
    for i in 0..window {
        recv_wr.wr_id = i as u64;
        unsafe { qp.post_receive([recv_wr])? };
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![WorkCompletion::default(); rx_depth.max(1)];

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
                recv_wr.wr_id = posted as u64;
                unsafe { qp.post_receive([recv_wr])? };
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
    Ok(Report::Peer)
}
