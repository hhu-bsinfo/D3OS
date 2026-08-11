//! Windowed one-way streaming with a deterministic per-message payload/header used to detect
//! loss, duplication, corruption and truncation. Ported from
//! `rust-rdma-bench/src/bench/accuracy.rs`; on RC this should always land at 100% (a self-check
//! of the harness), while UC is where loss/duplication/corruption actually become nonzero.
//!
//! The one D3OS-specific addition versus the Linux port: after polling a receive completion, the
//! CPU must flush the cache line(s) covering the received bytes before reading them for
//! verification, or it may observe stale cache contents instead of what the NIC DMA'd in (see
//! `rdma/mlx4/src/rdma_read.rs` for the precedent). Bandwidth/latency don't inspect payload
//! content, so they don't need this.

use crate::bench::{self, Role, IDLE_TIMEOUT_US};
use crate::comm::{AccuracyReport, Conn};
use crate::error::{other, Result};
use crate::report::Report;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use cpu_core::flush_cache;
use ibverbs::{ibv_wc, CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair};
use rdma::ibv_send_flags;
use time::get_time_in_us;

const HEADER_LEN: usize = 8;

/// One step of splitmix64, used as a deterministic PRNG so sender and receiver can each
/// independently (re)compute the expected payload for a given sequence number.
fn next_word(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// First `HEADER_LEN` bytes are the little-endian sequence number; the rest is a splitmix64
/// stream seeded from `seq`.
fn fill_payload(buf: &mut [u8], seq: u64) {
    buf[..HEADER_LEN].copy_from_slice(&seq.to_le_bytes());
    let mut state = seq;
    for chunk in buf[HEADER_LEN..].chunks_mut(8) {
        let word = next_word(&mut state).to_le_bytes();
        chunk.copy_from_slice(&word[..chunk.len()]);
    }
}

fn slot_range(slot: usize, msg_size: usize) -> Range<usize> {
    let start = slot * msg_size;
    start..start + msg_size
}

pub fn run(
    pd: &ProtectionDomain,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    role: Role,
    msg_size: usize,
    iterations: usize,
    tx_depth: usize,
) -> Result<Report> {
    if iterations == 0 {
        return Err(other("accuracy mode requires at least one iteration"));
    }
    if msg_size < HEADER_LEN {
        return Err(other("accuracy mode requires msg_size >= 8"));
    }

    let window = tx_depth.max(1).min(iterations);
    let mut mr = pd.allocate::<u8>(window * msg_size)?;

    match role {
        Role::Client => send(&mut mr, cq, qp, conn, msg_size, iterations, window),
        Role::Server => receive(&mut mr, cq, qp, conn, msg_size, iterations, window),
    }
}

fn send(
    mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
    window: usize,
) -> Result<Report> {
    for seq in 0..window {
        let range = slot_range(seq, msg_size);
        fill_payload(&mut mr[range.clone()], seq as u64);
        unsafe { qp.post_send(mr, vec![vec![range]], vec![seq as u64], vec![ibv_send_flags::SIGNALED])? };
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![ibv_wc::default(); window];

    conn.sync()?; // "ready" (defensive addition versus the Linux port, see receive()'s comment)

    while completed < iterations {
        let completions = cq.poll(&mut wc)?;
        let n = completions.len();
        for c in completions.iter() {
            bench::completion_error(c)?;
        }
        completed += n;
        // The send queue completes in order, so after `completed` completions every slot except
        // the `posted - completed` still outstanding is free to be refilled.
        for _ in 0..n {
            if posted < iterations {
                let slot = posted % window;
                let range = slot_range(slot, msg_size);
                fill_payload(&mut mr[range.clone()], posted as u64);
                unsafe { qp.post_send(mr, vec![vec![range]], vec![posted as u64], vec![ibv_send_flags::SIGNALED])? };
                posted += 1;
            }
        }
    }
    conn.sync()?; // "everything I was going to send has left the queue"

    let report: AccuracyReport = conn.recv_msg()?;
    Ok(Report::Accuracy(report))
}

fn receive(
    mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    msg_size: usize,
    iterations: usize,
    window: usize,
) -> Result<Report> {
    for slot in 0..window {
        let range = slot_range(slot, msg_size);
        unsafe { qp.post_receive(mr, vec![vec![range]], vec![slot as u64])? };
    }

    let mut report = AccuracyReport { msg_size, sent: iterations, ..AccuracyReport::default() };
    let mut seen = vec![false; iterations];
    let mut expected = vec![0u8; msg_size];
    let mut wc = vec![ibv_wc::default(); window];
    let mut batch: Vec<(usize, usize)> = Vec::with_capacity(window);

    // Not present in the Linux port's sketch, but harmless and closes a real race: without it
    // there's no guarantee the receive window above is posted before the sender's first burst
    // arrives.
    conn.sync()?; // "ready"

    let mut last_progress = get_time_in_us();
    while report.received < iterations {
        batch.clear();
        let completions = cq.poll(&mut wc)?;
        for c in completions.iter() {
            bench::completion_error(c)?;
            batch.push((c.wr_id() as usize, c.len()));
        }

        if batch.is_empty() {
            if get_time_in_us() - last_progress >= IDLE_TIMEOUT_US {
                break;
            }
            continue;
        }
        last_progress = get_time_in_us();
        report.received += batch.len();

        for &(slot, len) in &batch {
            let range = slot_range(slot, msg_size);
            unsafe { flush_cache(&mr[range.clone()]) };
            let got_len = len.min(msg_size);
            {
                let got = &mr[range.start..range.start + got_len];
                check(got, &mut expected, msg_size, &mut seen, &mut report);
            }
            unsafe { qp.post_receive(mr, vec![vec![range]], vec![slot as u64])? };
        }
    }
    report.lost = seen.iter().filter(|s| !**s).count();

    conn.sync()?; // matches the sender's "everything I was going to send has left the queue"
    conn.send_msg(&report)?;
    Ok(Report::Peer)
}

/// Classifies one received message: header-too-short/out-of-range sequence numbers are
/// unidentifiable, an already-seen sequence number is a duplicate (bailed out before byte
/// counting, so correct totals never exceed what was sent), otherwise it's checked for
/// truncation and XOR'd byte-by-byte against a freshly recomputed expected payload.
fn check(got: &[u8], expected: &mut [u8], msg_size: usize, seen: &mut [bool], report: &mut AccuracyReport) {
    if got.len() < HEADER_LEN {
        report.unidentifiable += 1;
        return;
    }
    let seq = u64::from_le_bytes(got[..HEADER_LEN].try_into().expect("checked above"));
    let Ok(seq_idx) = usize::try_from(seq) else {
        report.unidentifiable += 1;
        return;
    };
    if seq_idx >= seen.len() {
        report.unidentifiable += 1;
        return;
    }
    if seen[seq_idx] {
        report.duplicated += 1;
        return;
    }
    seen[seq_idx] = true;

    if got.len() < msg_size {
        report.truncated += 1;
    }
    fill_payload(expected, seq);

    let mut correct_bytes = 0u64;
    let mut correct_bits = 0u64;
    for (a, b) in got.iter().zip(expected.iter()) {
        let diff = a ^ b;
        if diff == 0 {
            correct_bytes += 1;
        }
        correct_bits += u64::from(8 - diff.count_ones());
    }
    if correct_bytes as usize != msg_size {
        report.corrupted += 1;
    }
    report.correct_bytes += correct_bytes;
    report.correct_bits += correct_bits;
}
