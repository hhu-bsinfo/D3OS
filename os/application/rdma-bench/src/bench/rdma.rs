//! One-sided RDMA WRITE/READ bandwidth benchmark. Unlike `bandwidth` (which is SEND/RECV and thus
//! symmetric), only the initiator's HCA ever produces a work completion here — the responder does
//! nothing but register a buffer, hand its address and rkey to the initiator, and wait. So "client"
//! and "server" no longer line up with "the side that measures something": the initiator is always
//! the client (the peer that opened the TCP connection and asked for this mode), and it is the only
//! side that reports numbers.
//!
//! Windowing works like `bandwidth`: `tx_depth`-many operations are kept outstanding at once, each
//! targeting its own `msg_size`-sized slot of a `window * msg_size` buffer on both ends, so a
//! completion for slot N frees it up to be reused before the whole `iterations` count is done.

use crate::bench::{self, Role};
use crate::comm::{Conn, RemoteBufferInfo};
use crate::error::Result;
use crate::report::{BandwidthStats, Report};
use alloc::vec;
use core::ops::Range;
use ibverbs::{ibv_wc, CompletionQueue, LocalMemoryRegion, ProtectionDomain, QueuePair, RemoteMemoryRegion};
use ibverbs::ffi::ibv_send_flags;
use time::get_time_in_us;

#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Write,
    Read,
}

pub fn run(
    direction: Direction,
    pd: &ProtectionDomain,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    role: Role,
    msg_size: usize,
    iterations: usize,
    tx_depth: usize,
) -> Result<Report> {
    let window = tx_depth.max(1).min(iterations);
    let mut mr = pd.allocate::<u8>(window * msg_size)?;

    match role {
        // The responder's own `cq`/`qp` go unused: RDMA WRITE/READ never consumes a receive
        // request and produces no completion on this side, so all it does is expose memory.
        Role::Server => respond(&mut mr, conn),
        Role::Client => {
            let RemoteBufferInfo { mut remote } = conn.recv_msg()?;
            initiate(direction, &mut mr, cq, qp, conn, &mut remote, msg_size, iterations, window)
        }
    }
}

fn respond(mr: &mut LocalMemoryRegion<u8>, conn: &Conn) -> Result<Report> {
    conn.send_msg(&RemoteBufferInfo { remote: mr.remote() })?;
    conn.sync()?; // "ready": the initiator is about to start
    conn.sync()?; // "done": the initiator's last operation has completed
    Ok(Report::Peer)
}

fn slot_range(slot: usize, msg_size: usize) -> Range<usize> {
    let start = slot * msg_size;
    start..start + msg_size
}

fn remote_slot_range(slot: usize, msg_size: usize) -> Range<u64> {
    let start = (slot * msg_size) as u64;
    start..start + msg_size as u64
}

fn post(
    direction: Direction,
    qp: &mut QueuePair,
    local_mr: &mut LocalMemoryRegion<u8>,
    remote_mr: &mut RemoteMemoryRegion<u8>,
    slot: usize,
    msg_size: usize,
    wr_id: u64,
) -> Result<()> {
    let local_range = slot_range(slot, msg_size);
    let remote_range = remote_slot_range(slot, msg_size);
    unsafe {
        match direction {
            Direction::Write => qp.rdma_write(
                local_mr,
                vec![vec![local_range]],
                remote_mr,
                vec![remote_range],
                vec![wr_id],
                vec![ibv_send_flags::SIGNALED],
            )?,
            Direction::Read => qp.rdma_read(
                remote_mr,
                vec![remote_range],
                local_mr,
                vec![vec![local_range]],
                vec![wr_id],
                vec![ibv_send_flags::SIGNALED],
            )?,
        }
    }
    Ok(())
}

fn initiate(
    direction: Direction,
    local_mr: &mut LocalMemoryRegion<u8>,
    cq: &CompletionQueue,
    qp: &mut QueuePair,
    conn: &Conn,
    remote_mr: &mut RemoteMemoryRegion<u8>,
    msg_size: usize,
    iterations: usize,
    window: usize,
) -> Result<Report> {
    conn.sync()?; // "ready"
    let t0 = get_time_in_us();

    for i in 0..window {
        post(direction, qp, local_mr, remote_mr, i, msg_size, i as u64)?;
    }

    let mut posted = window;
    let mut completed = 0usize;
    let mut wc = vec![ibv_wc::default(); window];

    while completed < iterations {
        let completions = cq.poll(&mut wc)?;
        let n = completions.len();
        for c in completions.iter() {
            bench::completion_error(c)?;
        }
        completed += n;
        for _ in 0..n {
            if posted < iterations {
                let slot = posted % window;
                post(direction, qp, local_mr, remote_mr, slot, msg_size, posted as u64)?;
                posted += 1;
            }
        }
    }

    let elapsed_us = get_time_in_us() - t0;
    conn.sync()?; // "done"

    Ok(Report::Bandwidth(BandwidthStats { msg_size, iterations, tx_depth: window, elapsed_us }))
}
