//! UC transport.
//!
//! Building the queue pair is the same shape as RC — the `QueuePairBuilder`/`handshake()` path
//! already gates every RC-only attribute (timeout, retry count, RNR timer, atomic depth) on
//! `qp_type`, so a UC queue pair moves through INIT/RTR/RTS with exactly the attribute mask UC
//! requires.
//!
//! What differs is the traffic: UC acknowledges nothing and retransmits nothing, so a message the
//! fabric drops is gone silently, with no completion on either side to mark it. Every benchmark
//! that waits on a peer therefore bounds that wait — see `bench::IDLE_TIMEOUT_US`.

use crate::error::Result;
use ibverbs::{ibv_qp_type::Type, CompletionQueue, PreparedQueuePair, ProtectionDomain};
use rdma::ibv_qp_cap;

pub fn build<'res>(
    pd: &'res ProtectionDomain<'res>,
    cq: &'res CompletionQueue<'res>,
    tx_depth: usize,
) -> Result<PreparedQueuePair<'res>> {
    let cap = ibv_qp_cap {
        max_send_wr: tx_depth as u32,
        max_recv_wr: tx_depth as u32,
        max_send_sge: 1,
        max_recv_sge: 1,
        max_inline_data: 0,
    };
    pd.create_qp(cq, cq, Type::IBV_QPT_UC, cap).build()
}
