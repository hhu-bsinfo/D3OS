pub mod accuracy;
pub mod bandwidth;
pub mod latency;
pub mod rdma;

use crate::cli::{Mode, Transport};
use crate::comm::Conn;
use crate::error::{other, Result};
use crate::report::Report;
use ibverbs::{CompletionQueue, ProtectionDomain, QueuePair, WorkCompletion};

/// How long a receive/wait loop will wait for progress from the peer before giving up. Needed
/// because UC acknowledges and retransmits nothing, so a message the fabric drops produces no
/// completion on either side to signal the loss.
pub const IDLE_TIMEOUT_US: usize = 2_000_000;

/// How long a bandwidth/latency run pauses, synchronized on both sides, right after the
/// handshake before starting its timed region.
///
/// A freshly-RTS queue pair has a one-time settling cost that, on the ib1/ib2 ConnectX-3
/// hardware, was confirmed (on the `rust-rdma-bench` Linux side) to swamp a short run's *entire*
/// measured throughput rather than just its first sample — a bandwidth run with no warm-up
/// measured ~15-30x lower throughput than a reference tool on the identical path. This is a
/// genuinely time-bound cost, not a "number of messages" one: a discarded warm-up batch bounded
/// by queue depth finishes in well under a millisecond even at the slow cold rate, nowhere near
/// enough elapsed time to matter — only an actual pause of this rough magnitude fixed it in
/// testing.
pub const WARMUP_SETTLE_MS: usize = 100;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

/// The authority on which (transport, mode) combinations are usable, checked during the
/// handshake so an unimplemented combination is rejected before any RDMA resources are built.
pub fn supported(transport: Transport, mode: Mode) -> bool {
    match (transport, mode) {
        (Transport::Ud, _) => false,
        // RDMA READ is not in UC's transport-service repertoire (IBTA 1.2.1, table 44) — UC has
        // RDMA WRITE but no read/atomics.
        (Transport::Uc, Mode::RdmaRead) => false,
        (Transport::Rc | Transport::Uc, _) => true,
    }
}

pub fn completion_error(wc: &WorkCompletion) -> Result<()> {
    if let Some((status, vendor_err)) = wc.error() {
        terminal::println!("work completion error: {:?} vendor_err={}", status, vendor_err);
        return Err(other("work completion error"));
    }
    Ok(())
}

pub fn run(
    mode: Mode,
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
    match mode {
        Mode::Bandwidth => bandwidth::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth, rx_depth),
        Mode::Latency => latency::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth, rx_depth),
        Mode::Accuracy => accuracy::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth, rx_depth),
        Mode::RdmaWrite => {
            rdma::run(rdma::Direction::Write, pd, cq, qp, conn, role, msg_size, iterations, tx_depth, rx_depth)
        }
        Mode::RdmaRead => {
            rdma::run(rdma::Direction::Read, pd, cq, qp, conn, role, msg_size, iterations, tx_depth, rx_depth)
        }
    }
}
