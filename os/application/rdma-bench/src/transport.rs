use crate::cli::{Mode, Transport};
use crate::error::Result;
use ibverbs::{CompletionQueue, PreparedQueuePair, ProtectionDomain, QueuePairType};

/// Builds a queue pair of the requested transport type, ready to be handshaked with a remote
/// endpoint.
pub fn build<'res>(
    transport: Transport,
    mode: Mode,
    pd: &'res ProtectionDomain<'res>,
    cq: &'res CompletionQueue,
    tx_depth: usize,
    rx_depth: usize,
) -> Result<PreparedQueuePair<'res>> {
    let qp_type = match transport {
        Transport::Rc => QueuePairType::RC,
        Transport::Uc => QueuePairType::UC,
        Transport::Ud => unimplemented!("UD transport not yet implemented (see module doc comment)"),
    };

    let mut builder = pd.create_qp(cq, cq, qp_type);

    builder
        .set_max_send_wr(tx_depth as u32)
        .set_max_recv_wr(rx_depth as u32);

    if let Mode::RdmaWrite | Mode::RdmaRead = mode {
        builder.allow_remote_rw();
    }

    builder.build()
}
