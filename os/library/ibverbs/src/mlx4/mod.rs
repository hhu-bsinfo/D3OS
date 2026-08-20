use alloc::vec::Vec;
use spin::Mutex;
use rdma::{ibv_qp_cap, ibv_qp_state, ibv_qp_type};
use rdma::uverbs_uapi::{ReceiveWorkRequest, SendWorkRequest};

pub(super) mod completion_queue;
mod queue_pair;

use queue_pair::QueuePair;

/// A per-device registry of live queue pairs, shared between whichever `ibv_qp`s and `ibv_cq`s
/// were created against this device.
///
/// Posting (`post_send`/`post_recv`) reaches a QP directly through the `ibv_qp` that owns it,
/// but resolving a completion (`CompletionQueue::poll`) only has the CQE's `qp_number` to go on
/// and needs to reach *some other* QP's `WorkQueue` state — this registry is what makes that
/// possible, mirroring the "find by number in a `Vec`" lookup that used to live in the kernel's
/// `ConnectX3Nic::qps` before posting and polling moved out here.
pub struct Device {
    device_handle: usize,
    /// Retrieved from QUERY_DEV_CAP -> log_max_qp_sz
    /// Maximum size of Send Queue in WQEBB (including SQ Headroom) or Receive Queue in WQE is 2^log_max_qp_size.
    // TODO: query the device for these instead of hardcoding them; nothing surfaces
    // QUERY_DEV_CAP to userspace yet.
    log_max_qp_size: u8,
    log_max_rq_sge: u8,
    log_max_sq_sge: u8,
    max_wqe_sq_size: u16,
    qps: Mutex<Vec<QueuePair>>,
}

impl Device {
    pub fn new(device_handle: usize) -> Self {
        Self {
            device_handle,
            log_max_qp_size: 16,
            log_max_rq_sge: 5,
            log_max_sq_sge: 5,
            max_wqe_sq_size: 1024,
            qps: Mutex::new(Vec::new()),
        }
    }

    pub fn create_qp(&self, send_cq_num: u32, recv_cq_num: u32, qp_type: ibv_qp_type::Type, cap: ibv_qp_cap) -> Result<u32, &'static str> {
        let qp = QueuePair::create(self, send_cq_num, recv_cq_num, qp_type, cap)?;
        let number = qp.number;
        self.qps.lock().push(qp);
        Ok(number)
    }

    pub fn post_send(&self, qp_num: u32, wrs: &[SendWorkRequest]) -> Result<(), &'static str> {
        let mut qps = self.qps.lock();
        let qp = qps.iter_mut().find(|qp| qp.number == qp_num).ok_or("invalid queue pair number")?;
        qp.post_send(wrs)
    }

    pub fn post_receive(&self, qp_num: u32, wrs: &[ReceiveWorkRequest]) -> Result<(), &'static str> {
        let mut qps = self.qps.lock();
        let qp = qps.iter_mut().find(|qp| qp.number == qp_num).ok_or("invalid queue pair number")?;
        qp.post_receive(wrs)
    }

    /// Record a queue pair's new state after a successful `ibv_modify_qp`.
    ///
    /// `ibv_modify_qp` is control-path and still goes through the kernel via `uverbs()`, which
    /// updates the kernel's own `QueuePair::state` — but `post_send`/`post_recv` are checked
    /// against *this* registry's copy, since posting no longer talks to the kernel at all. Without
    /// this the state here would stay stuck at `IBV_QPS_RESET` forever and every post would be
    /// rejected as "queue pair cannot send/receive in this state" even after the QP reached RTS.
    pub fn set_qp_state(&self, qp_num: u32, state: ibv_qp_state) -> Result<(), &'static str> {
        let mut qps = self.qps.lock();
        let qp = qps.iter_mut().find(|qp| qp.number == qp_num).ok_or("invalid queue pair number")?;
        qp.set_state(state);
        Ok(())
    }

    /// Resolve a CQE against the queue pair it belongs to: check the reported WQE index against
    /// what the driver expected, advance that queue's tail by the chain size, and return the
    /// `wr_id` to report in the work completion.
    pub(super) fn resolve_completion(&self, qp_num: u32, wqe_index: u32, is_send: bool) -> Option<u64> {
        let mut qps = self.qps.lock();
        let qp = qps.iter_mut().find(|qp| qp.number == qp_num)?;
        qp.check_wqe_index(wqe_index, is_send);
        let chain_size = qp.query_chain_size(wqe_index as usize, is_send);
        if is_send {
            qp.advance_send_queue_by(chain_size);
        } else {
            qp.advance_receive_queue_by(chain_size);
        }
        Some(qp.query_wr_id(wqe_index as usize, is_send))
    }
}
