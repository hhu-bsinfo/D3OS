//! UD transport: deliberately not implemented.
//!
//! D3OS's `PreparedQueuePair::handshake` unconditionally applies RC/UC-style
//! `dest_qp_num`/`ah_attr` logic even when `qp_type` is UD (same "TODO: this is only valid for RC
//! and UC" comment carried over from upstream `rust-ibverbs`), and the kernel's `UverbsCmd` enum
//! has no `CreateAh`/`DestroyAh` command at all — so there is no way to build an address handle for
//! a UD send. Building UD support here would mean adding both a real UD handshake path and AH
//! support, either in `ibverbs`/`rdma` or via raw uverbs calls — out of scope for now. Mirrors
//! `rust-rdma-bench/src/transport/ud.rs` on the Linux side, which is blocked for the same reasons.

use crate::error::Result;
use ibverbs::{CompletionQueue, PreparedQueuePair, ProtectionDomain};

pub fn build<'res>(
    _pd: &'res ProtectionDomain<'res>,
    _cq: &'res CompletionQueue<'res>,
    _tx_depth: usize,
) -> Result<PreparedQueuePair<'res>> {
    unimplemented!("UD transport not yet implemented (see module doc comment)")
}
