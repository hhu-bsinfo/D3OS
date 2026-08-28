//! Stop-and-wait ping/pong latency benchmark. Ported from
//! `rust-rdma-bench/src/bench/latency.rs` — exactly one message in flight per direction,
//! `tx_depth` is accepted (to match the shared `bench::run` dispatch signature) but unused.

use crate::bench::{self, Role, IDLE_TIMEOUT_US};
use crate::comm::Conn;
use crate::error::Result;
use crate::report::{LatencyStats, Report};
use alloc::vec;
use alloc::vec::Vec;
use ibverbs::{ibv_wc, CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair};
use ibverbs::ffi::ibv_send_flags;
use time::get_time_in_us;

const WR_SEND: u64 = 1;
const WR_RECV: u64 = 2;

pub fn run(
    pd: &ProtectionDomain,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    role: Role,
    msg_size: usize,
    iterations: usize,
    _tx_depth: usize,
) -> Result<Report> {
    // Two separate buffers: reusing one for both directions would let the echo overwrite bytes
    // the outgoing send is still reading.
    let mut send_mr = pd.allocate::<u8>(msg_size)?;
    let mut recv_mr = pd.allocate::<u8>(msg_size)?;

    match role {
        Role::Client => ping(&mut send_mr, &mut recv_mr, cq, qp, conn, msg_size, iterations),
        Role::Server => pong(&mut send_mr, &mut recv_mr, cq, qp, conn, msg_size, iterations),
    }
}

/// Waits for the bitmask of outstanding wr_ids in `want` to clear, or for `IDLE_TIMEOUT_US` to
/// elapse. Returns the still-pending subset of `want` — zero means fully satisfied.
fn wait_for(cq: &CompletionQueue, wc: &mut [ibv_wc], want: u64) -> Result<u64> {
    let mut pending = want;
    let deadline = get_time_in_us() + IDLE_TIMEOUT_US;
    while pending != 0 {
        let completions = cq.poll(wc)?;
        for c in completions.iter() {
            bench::completion_error(c)?;
            pending &= !c.wr_id();
        }
        if pending != 0 && get_time_in_us() >= deadline {
            break;
        }
    }
    Ok(pending)
}

fn ping(
    send_mr: &mut LocalMemoryRegion<u8>,
    recv_mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
) -> Result<Report> {
    let mut wc = vec![ibv_wc::default(); 4];
    let mut samples: Vec<f64> = Vec::with_capacity(iterations);

    unsafe { qp.post_receive(recv_mr, vec![vec![0..msg_size]], vec![WR_RECV])? };
    conn.sync()?; // both sides have a receive posted

    for i in 0..iterations {
        let t0 = get_time_in_us();
        unsafe { qp.post_send(send_mr, vec![vec![0..msg_size]], vec![WR_SEND], vec![ibv_send_flags::SIGNALED])? };

        if wait_for(cq, &mut wc, WR_SEND | WR_RECV)? != 0 {
            terminal::println!(
                "round trip {} timed out after {} us; {} of {} iterations skipped",
                i,
                IDLE_TIMEOUT_US,
                iterations - i,
                iterations
            );
            break;
        }
        // Half round trip, in microseconds.
        samples.push((get_time_in_us() - t0) as f64 / 2.0);

        if i + 1 < iterations {
            unsafe { qp.post_receive(recv_mr, vec![vec![0..msg_size]], vec![WR_RECV])? };
        }
    }
    conn.sync()?; // both sides done

    Ok(Report::Latency(LatencyStats::from_samples(msg_size, &samples)))
}

fn pong(
    send_mr: &mut LocalMemoryRegion<u8>,
    recv_mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
) -> Result<Report> {
    let mut wc = vec![ibv_wc::default(); 4];
    let mut echoed = 0usize;

    unsafe { qp.post_receive(recv_mr, vec![vec![0..msg_size]], vec![WR_RECV])? };
    conn.sync()?; // both sides have a receive posted

    for _ in 0..iterations {
        if wait_for(cq, &mut wc, WR_RECV)? != 0 {
            break;
        }
        // Repost the receive before echoing so the next ping's receive is armed ahead of time.
        unsafe { qp.post_receive(recv_mr, vec![vec![0..msg_size]], vec![WR_RECV])? };
        unsafe { qp.post_send(send_mr, vec![vec![0..msg_size]], vec![WR_SEND], vec![ibv_send_flags::SIGNALED])? };
        wait_for(cq, &mut wc, WR_SEND)?;
        echoed += 1;
    }
    conn.sync()?; // both sides done

    terminal::println!("echoed {} of {} messages", echoed, iterations);
    Ok(Report::Peer)
}
