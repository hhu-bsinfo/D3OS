//! Stop-and-wait ping/pong latency benchmark. Ported from
//! `rust-rdma-bench/src/bench/latency.rs` — exactly one message in flight per direction,
//! `tx_depth` is accepted (to match the shared `bench::run` dispatch signature) but unused.

use crate::bench::{self, Role, IDLE_TIMEOUT_US};
use crate::comm::Conn;
use crate::error::Result;
use alloc::vec;
use alloc::vec::Vec;
use ibverbs::{ibv_wc, CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair};
use rdma::ibv_send_flags;
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
) -> Result<()> {
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
) -> Result<()> {
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

    report(msg_size, &samples);
    Ok(())
}

fn pong(
    send_mr: &mut LocalMemoryRegion<u8>,
    recv_mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
) -> Result<()> {
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
    Ok(())
}

/// Nearest-rank percentiles over sorted samples, matching `perftest`'s `ib_send_lat` layout.
fn report(msg_size: usize, samples: &[f64]) {
    if samples.is_empty() {
        terminal::println!("no samples collected");
        return;
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);

    let n = sorted.len();
    let avg = sorted.iter().sum::<f64>() / n as f64;
    let stdev = if n > 1 {
        libm::sqrt(sorted.iter().map(|s| libm::pow(s - avg, 2f64)).sum::<f64>() / (n - 1) as f64)
    } else {
        0.0
    };
    let percentile = |p: f64| sorted[(libm::ceil((n as f64) * p) as usize).clamp(1, n) - 1];

    terminal::println!(
        "{:>8}  {:>12}  {:>12}  {:>12}  {:>16}  {:>12}  {:>14}  {:>10}  {:>12}",
        "#bytes", "#iterations", "t_min[usec]", "t_max[usec]", "t_typical[usec]", "t_avg[usec]", "t_stdev[usec]", "99%[usec]", "99.9%[usec]"
    );
    terminal::println!(
        "{:>8}  {:>12}  {:>12.2}  {:>12.2}  {:>16.2}  {:>12.2}  {:>14.2}  {:>10.2}  {:>12.2}",
        msg_size,
        n,
        sorted[0],
        sorted[n - 1],
        percentile(0.50),
        avg,
        stdev,
        percentile(0.99),
        percentile(0.999)
    );
}
