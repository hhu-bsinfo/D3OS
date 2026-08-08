use crate::error::Result;
use ibverbs::{ibv_qp_type::Type, CompletionQueue, PreparedQueuePair, ProtectionDomain};
use rdma::ibv_qp_cap;

/// Builds an RC queue pair with the given send/receive depth, ready to be handed a remote
/// endpoint via `PreparedQueuePair::handshake`.
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
    pd.create_qp(cq, cq, Type::IBV_QPT_RC, cap).build()
}
