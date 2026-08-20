use crate::cli::{Mode, Transport};
use crate::error::Result;
use ibverbs::{CompletionQueue, PreparedQueuePair, ProtectionDomain};
use rdma::ibv_qp_cap;
use rdma::ibv_qp_type::Type;

/// Builds a queue pair of the requested transport type, ready to be handshaked with a remote
/// endpoint.
pub fn build<'res>(
    transport: Transport,
    mode: Mode,
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

    let qp_type = match transport {
        Transport::Rc => Type::IBV_QPT_RC,
        Transport::Uc => Type::IBV_QPT_UC,
        Transport::Ud => unimplemented!("UD transport not yet implemented (see module doc comment)"),
    };

    let mut builder = pd.create_qp(cq, cq, qp_type, cap);

    if let Mode::RdmaWrite | Mode::RdmaRead = mode {
        builder.allow_remote_rw();
    }

    builder.build()
}
