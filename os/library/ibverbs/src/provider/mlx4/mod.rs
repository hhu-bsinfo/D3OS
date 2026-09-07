use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core3::io;
use core3::io::{Error, ErrorKind};
use rdma::uverbs_uapi::{CreateMrRequest, CreateMrResponse, AllocPdResponse, DeallocPdRequest, QueryPortRequest, UserSlice, UverbsCmd};
use rdma::ib_core::{ibv_access_flags, ibv_device_attr, ibv_gid, ibv_port_attr};
use spin::Mutex;

pub(crate) mod completion_queue;
mod queue_pair;

use crate::cmd::uverbs;
use crate::provider::mlx4::completion_queue::CompletionQueue;
use crate::provider::{IbvCompletionQueue, IbvContext, IbvQueuePair, QpInitAttr};
use crate::MemoryRegionMetadata;
use queue_pair::QueuePair;

/// A per-device registry of live queue pairs, shared between whichever `ibv_qp`s and `ibv_cq`s
/// were created against this device.
///
/// Posting (`post_send`/`post_recv`) reaches a QP directly through the `ibv_qp` that owns it,
/// but resolving a completion (`CompletionQueue::poll`) only has the CQE's `qp_number` to go on
/// and needs to reach *some other* QP's `WorkQueue` state — this registry is what makes that
/// possible, mirroring the "find by number in a `Vec`" lookup that used to live in the kernel's
/// `ConnectX3Nic::qps` before posting and polling moved out here.
pub struct Mlx4Context {
    device_handle: usize,
    /// Retrieved from QUERY_DEV_CAP -> log_max_qp_sz
    /// Maximum size of Send Queue in WQEBB (including SQ Headroom) or Receive Queue in WQE is 2^log_max_qp_size.
    // TODO: query the device for these instead of hardcoding them; nothing surfaces
    // QUERY_DEV_CAP to userspace yet.
    log_max_qp_size: u8,
    log_max_rq_sge: u8,
    log_max_sq_sge: u8,
    max_wqe_sq_size: u16,
    qps: Mutex<Vec<Arc<QueuePair>>>,
}

impl Mlx4Context {
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

    /// Resolve a CQE against the queue pair it belongs to: check the reported WQE index against
    /// what the driver expected, advance that queue's tail by the chain size, and return the
    /// `wr_id` to report in the work completion.
    pub(crate) fn resolve_completion(&self, qp_num: u32, wqe_index: u32, is_send: bool) -> Option<u64> {
        let mut qps = self.qps.lock();
        let qp = qps.iter_mut().find(|qp| qp.number == qp_num)?;
        qp.resolve_completion(wqe_index, is_send)
    }

    #[inline(always)]
    pub(super) fn device_handle(&self) -> usize {
        self.device_handle
    }
}

impl IbvContext for Mlx4Context {
    fn query_device(&self) -> io::Result<ibv_device_attr> {
        let mut resp = MaybeUninit::<ibv_device_attr>::uninit();
        uverbs(self.device_handle, UverbsCmd::QueryDevice, UserSlice::EMPTY, UserSlice::from_mut(&mut resp))?;
        Ok(unsafe { resp.assume_init() })
    }

    fn query_port(&self, port_num: u8) -> io::Result<ibv_port_attr> {
        let req = QueryPortRequest { port_num };
        let mut resp = MaybeUninit::<ibv_port_attr>::uninit();
        uverbs(self.device_handle, UverbsCmd::QueryPort, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp))?;
        Ok(unsafe { resp.assume_init() })
    }

    fn query_gid(&self, _port_num: u8, _index: i32) -> io::Result<ibv_gid> {
        // TODO: figure out how to actually do this as the Nautilus driver can't
        Ok(ibv_gid { raw: [0; 16] })
    }

    fn create_qp(self: Arc<Self>, pd: u32, attr: &QpInitAttr) -> io::Result<Arc<dyn IbvQueuePair>> {
        let qp = Arc::new(QueuePair::create(self.clone(), pd, attr)?);
        self.qps.lock().push(qp.clone());
        Ok(qp)
    }

    fn create_cq(self: Arc<Self>, min_cpe: i32, _cq_context: isize, channel: Option<()>, comp_vector: i32) -> io::Result<Box<dyn IbvCompletionQueue>> {
        assert!(channel.is_none());
        assert_eq!(comp_vector, 0);

        let cq = CompletionQueue::create(self, min_cpe)?;
        Ok(Box::new(cq))
    }

    fn alloc_pd(&self) -> io::Result<u32> {
        let mut resp = MaybeUninit::<AllocPdResponse>::uninit();
        uverbs(self.device_handle, UverbsCmd::AllocPd, UserSlice::EMPTY, UserSlice::from_mut(&mut resp))?;
        let resp = unsafe { resp.assume_init() };
        Ok(resp.pd)
    }

    fn dealloc_pd(&self, pd: u32) -> io::Result<()> {
        let req = DeallocPdRequest { pd };
        uverbs(self.device_handle, UverbsCmd::DeallocPd, UserSlice::from_ref(&req), UserSlice::EMPTY)?;
        Ok(())
    }

    fn reg_mr(&self, pd: u32, ptr: *mut u8, len: usize, access: ibv_access_flags) -> io::Result<MemoryRegionMetadata> {
        if len == 0 {
            return Err(Error::from(ErrorKind::InvalidInput))
        }

        let req = CreateMrRequest {
            pd,
            ibv_access_flags: access,
            data_ptr: ptr,
            len,
        };

        let mut resp = MaybeUninit::<CreateMrResponse>::uninit();
        uverbs(self.device_handle, UverbsCmd::RegMr, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp))?;
        let CreateMrResponse { handle, lkey, rkey } = unsafe { resp.assume_init() };
        Ok(MemoryRegionMetadata {
            handle,
            lkey,
            rkey,
        })
    }

    fn dereg_mr(&self, meta: MemoryRegionMetadata) {
        uverbs(self.device_handle, UverbsCmd::DeregMr, UserSlice::from_ref(&meta.handle), UserSlice::EMPTY)
            .expect("failed to destroy memory region");
    }
}
