use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core3::io;
use core3::io::{Error, ErrorKind};
use rdma::ib_core::{PortAttr, AccessFlags, DeviceAttr};
use rdma::uverbs_uapi::{CreateMrRequest, CreateMrResponse, AllocPdResponse, DeallocPdRequest, QueryPortRequest, UserSlice, UverbsCmd, OpenDeviceResponse};
use spin::Mutex;
use tock_registers::{register_bitfields, register_fields, register_structs};
use tock_registers::interfaces::Writeable;
use tock_registers::registers::WriteOnly;

pub(crate) mod completion_queue;
mod queue_pair;

use crate::cmd::uverbs;
use crate::provider::mlx4::completion_queue::CompletionQueue;
use crate::provider::{IbvCompletionQueue, IbvContext, IbvQueuePair, QpInitAttr};
use crate::{Gid, MemoryRegionMetadata};
use queue_pair::QueuePair;
use rdma::ProtectionDomainHandle;

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
    uar_index: u32,
    doorbell_page: NonNull<DoorbellPage>,
    blueflame_page: *mut u8,
    log_max_qp_size: u8,
    log_max_rq_sge: u8,
    log_max_sq_sge: u8,
    max_wqe_sq_size: u16,
    qps: Mutex<Vec<Arc<QueuePair>>>,
}

impl Mlx4Context {
    pub fn new(device_handle: usize) -> io::Result<Self> {
        let mut resp = MaybeUninit::<OpenDeviceResponse>::uninit();
        uverbs(device_handle, UverbsCmd::OpenDevice, UserSlice::EMPTY, UserSlice::from_mut(&mut resp))?;
        let resp = unsafe { resp.assume_init() };
        let doorbell_page = NonNull::new(resp.doorbell_page.cast())
            .ok_or(Error::new(ErrorKind::Other, "Doorbell page not mapped"))?;
        Ok(Self {
            device_handle,
            uar_index: resp.uar_index,
            doorbell_page,
            blueflame_page: resp.blueflame_page,
            log_max_qp_size: 16,
            log_max_rq_sge: 5,
            log_max_sq_sge: 5,
            max_wqe_sq_size: 1024,
            qps: Mutex::new(Vec::new()),
        })
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

    pub(super) fn ring_qp_doorbell(&self, qp_number: u32) {
        assert!(qp_number < 1 << 24);
        let doorbell_page = unsafe { self.doorbell_page.as_ptr().as_mut() }.unwrap();
        let value = (qp_number << 8).to_be();
        doorbell_page.qp_doorbell.set(value);
    }

    pub(super) fn ring_cq_doorbell(&self, cq_number: u32, cmd: CqArmCmd, cmd_sn: u8, cq_consumer_index: u32) {
        assert!(cq_number < 1 << 24);
        assert!(cmd_sn < 1 << 3);
        assert!(cq_consumer_index < 1 << 24);
        let doorbell_page = unsafe { self.doorbell_page.as_ptr().as_mut() }.unwrap();
        let value = CqDoorbellRegister::CQ_NUM.val((cq_number.to_be() >> 8) as u64)
            + CqDoorbellRegister::CMD.val(cmd as u64)
            + CqDoorbellRegister::CMD_SN.val(cmd_sn as u64)
            + CqDoorbellRegister::CQ_CI.val((cq_consumer_index.to_be() >> 8) as u64);
        doorbell_page.cq_doorbell.write(value);
    }
}

impl IbvContext for Mlx4Context {
    fn query_device(&self) -> io::Result<DeviceAttr> {
        let mut resp = MaybeUninit::<DeviceAttr>::uninit();
        uverbs(self.device_handle, UverbsCmd::QueryDevice, UserSlice::EMPTY, UserSlice::from_mut(&mut resp))?;
        Ok(unsafe { resp.assume_init() })
    }

    fn query_port(&self, port_num: u8) -> io::Result<PortAttr> {
        let req = QueryPortRequest { port_num };
        let mut resp = MaybeUninit::<PortAttr>::uninit();
        uverbs(self.device_handle, UverbsCmd::QueryPort, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp))?;
        Ok(unsafe { resp.assume_init() })
    }

    fn query_gid(&self, _port_num: u8, _index: i32) -> io::Result<Gid> {
        // TODO: figure out how to actually do this as the Nautilus driver can't
        Ok(Gid { raw: [0; 16] })
    }

    fn create_qp(self: Arc<Self>, pd: ProtectionDomainHandle, attr: &QpInitAttr) -> io::Result<Arc<dyn IbvQueuePair>> {
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

    fn alloc_pd(&self) -> io::Result<ProtectionDomainHandle> {
        let mut resp = MaybeUninit::<AllocPdResponse>::uninit();
        uverbs(self.device_handle, UverbsCmd::AllocPd, UserSlice::EMPTY, UserSlice::from_mut(&mut resp))?;
        let resp = unsafe { resp.assume_init() };
        Ok(resp.pd)
    }

    fn dealloc_pd(&self, pd: ProtectionDomainHandle) -> io::Result<()> {
        let req = DeallocPdRequest { pd };
        uverbs(self.device_handle, UverbsCmd::DeallocPd, UserSlice::from_ref(&req), UserSlice::EMPTY)?;
        Ok(())
    }

    fn reg_mr(&self, pd: ProtectionDomainHandle, ptr: *mut u8, len: usize, access: AccessFlags) -> io::Result<MemoryRegionMetadata> {
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

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
enum CqArmCmd {
    ArmSolicit = 1,
    ArmNext = 2,
}

// CQ Doorbell Register
// BE|63      |55      |47      |39      |31      |23      |15      |7      0|
//   |--------|    CQ Consumer Index     |CMD & SN|      CQ Number           |
//
// CQ Consumer Index and CQ Number are in BigEndian Format

register_bitfields![u64,
    pub CqDoorbellRegister [
        CQ_NUM OFFSET(0)  NUMBITS(24),
        CMD    OFFSET(24) NUMBITS(2),
        CMD_SN OFFSET(28) NUMBITS(2),
        CQ_CI  OFFSET(32) NUMBITS(24),
    ]
];

register_structs! {
    pub DoorbellPage {
    (0x000 => _reserved1),
    (0x014 => pub qp_doorbell: WriteOnly<u32>),
    (0x018 => _reserved2),
    (0x020 => pub cq_doorbell: WriteOnly<u64, CqDoorbellRegister::Register>),
    (0x028 => _reserved3),
    (0x1000 => @END),
    }
}

