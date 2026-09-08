pub mod accuracy;
pub mod bandwidth;
pub mod latency;
pub mod rdma;

use crate::cli::{Mode, Transport};
use crate::comm::Conn;
use crate::error::{other, Result};
use crate::report::Report;
use ibverbs::{CompletionQueue, ProtectionDomain, QueuePair};
use ibverbs::completion_queue::WorkCompletion;

/// How long a receive/wait loop will wait for progress from the peer before giving up. Needed
/// because UC acknowledges and retransmits nothing, so a message the fabric drops produces no
/// completion on either side to signal the loss.
pub const IDLE_TIMEOUT_US: usize = 2_000_000;

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
) -> Result<Report> {
    match mode {
        Mode::Bandwidth => bandwidth::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth),
        Mode::Latency => latency::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth),
        Mode::Accuracy => accuracy::run(pd, cq, qp, conn, role, msg_size, iterations, tx_depth),
        Mode::RdmaWrite => {
            rdma::run(rdma::Direction::Write, pd, cq, qp, conn, role, msg_size, iterations, tx_depth)
        }
        Mode::RdmaRead => {
            rdma::run(rdma::Direction::Read, pd, cq, qp, conn, role, msg_size, iterations, tx_depth)
        }
    }
}
