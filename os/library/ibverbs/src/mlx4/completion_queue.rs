//! Userspace-owned completion queue: creation still goes through the kernel (it needs to build
//! an MTT for the CQE buffer and run the `Sw2HwCq` CMD-interface transition), but polling and
//! CQE parsing happen entirely against the mapped buffer from here on, without a syscall per
//! poll. Arming goes through the UAR page the kernel maps into this process at creation, so it
//! needs no syscall either. This mirrors the kernel's former
//! `os/kernel/src/device/mlx4/completion_queue.rs` `poll`/`poll_one`/`get_next_cqe_sw`/`arm`,
//! which were deleted from the kernel once this moved here.

use core::mem::MaybeUninit;
use core::sync::atomic::{compiler_fence, Ordering};

use log::{error, warn};
use modular_bitfield_msb::{
    bitfield,
    prelude::{B4, B7, B12},
    specifiers::{B5, B24},
};
use mm::{mmap, MmapFlags, PAGE_SIZE};
use rdma::{ibv_wc, ibv_wc_flags, ibv_wc_opcode, ibv_wc_status};
use rdma::uverbs_uapi::{CreateCqRequest, CreateCqResponse, UserSlice};
use rdma::uverbs_uapi::UverbsCmd::{CreateCq, DrainEvents};
use strum_macros::FromRepr;
use tock_registers::interfaces::Writeable;
use tock_registers::registers::WriteOnly;

use crate::ffi::uverbs;

use super::Device;
use super::queue_pair::{DoorbellPage, QueuePairOpcode};

/// Size in bytes of a hardware completion queue entry. CX3 also supports a 64 B format, but this
/// driver always uses the 32 B one.
const CQE_SIZE: usize = 32;

/// How many polls between rate-limited event-queue drains.
///
/// Draining the event queue used to piggyback on every uverbs syscall (`uverbs_ctl` called
/// `uverbs_drain_events` after every verb); now that polling never goes through a syscall, a
/// port-down/QP-error/internal-error notification would otherwise never get noticed during a
/// tight polling loop. This calls `UverbsCmd::DrainEvents` itself instead, at the same interval
/// the kernel used to check its internal error buffer on.
const DRAIN_EVENTS_INTERVAL: u32 = 4096;

pub struct CompletionQueue {
    device_handle: usize,
    number: u32,
    num_entries: u32,
    buffer: &'static mut [u8],
    doorbell: *mut CompletionQueueDoorbell,
    doorbell_page: *mut DoorbellPage,
    arm_sequence_number: u32,
    consumer_index: u32,
    poll_count: u32,
}

impl CompletionQueue {
    /// Create a new completion queue with at least `min_num_entries` entries.
    ///
    /// This is used by ibv_create_cq.
    pub fn create(device_handle: usize, min_num_entries: i32) -> Result<Self, &'static str> {
        let num_entries = u32::try_from(min_num_entries).map_err(|_| "cq_entries must be positive")?.next_power_of_two().max(1);
        let size = num_entries as usize * CQE_SIZE;
        let buffer = mmap(0, size.next_multiple_of(PAGE_SIZE), MmapFlags::empty()).map_err(|_| "failed to allocate CQE buffer")?;
        buffer.fill(0);

        let doorbell_ptr: *mut CompletionQueueDoorbell = mmap(0, size_of::<CompletionQueueDoorbell>(), MmapFlags::empty())
            .map_err(|_| "failed to allocate CQ doorbell")?
            .as_mut_ptr()
            .cast();
        unsafe {
            (*doorbell_ptr).update_consumer_index.set(0);
            (*doorbell_ptr).arm_consumer_index.set(0);
        }

        let req = CreateCqRequest {
            cq_entries: num_entries.try_into().unwrap(),
            buffer: buffer.as_ptr(),
            doorbell_ptr: doorbell_ptr.cast(),
        };
        let mut resp = MaybeUninit::<CreateCqResponse>::uninit();
        let resp = match uverbs(device_handle, CreateCq, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp)) {
            Ok(_) => unsafe { resp.assume_init() },
            Err(_) => return Err("could not create cq"),
        };

        let mut cq = Self {
            device_handle,
            number: resp.cq_num,
            num_entries,
            buffer,
            doorbell: doorbell_ptr,
            doorbell_page: resp.doorbell_page.cast(),
            arm_sequence_number: 1,
            consumer_index: 0,
            poll_count: 0,
        };
        // The event-driven completion model this enables isn't used by this driver (everything
        // polls), but the initial arm matches what the reference driver does and costs nothing on
        // the polling hot path.
        cq.arm();
        Ok(cq)
    }

    /// Arm this completion queue by writing the consumer index to the doorbell record and then
    /// ringing the UAR doorbell.
    ///
    /// This is used by ibv_req_notify_cq.
    pub fn arm(&mut self) {
        const _DOORBELL_REQUEST_NOTIFICATION_SOLICITED: u32 = 0x1;
        const DOORBELL_REQUEST_NOTIFICATION: u32 = 0x2;
        let sn = self.arm_sequence_number & 3;
        let ci = self.consumer_index & 0xffffff;
        let cmd = DOORBELL_REQUEST_NOTIFICATION;
        unsafe { &*self.doorbell }.arm_consumer_index.set((sn << 28 | cmd << 24 | ci).to_be());
        // Make sure that the doorbell record in host memory is
        // written before ringing the doorbell via PCI MMIO.
        compiler_fence(Ordering::SeqCst);
        let doorbell_page = unsafe { &*self.doorbell_page };
        doorbell_page.cq_sn_cmd_num.set((sn << 28 | cmd << 24 | self.number).to_be());
        doorbell_page.cq_consumer_index.set(ci.to_be());
    }

    /// Get the number of this completion queue.
    pub fn number(&self) -> u32 {
        self.number
    }

    /// Poll this completion queue and return the number of new completions.
    ///
    /// This is used by ibv_poll_cq. `device` is the shared registry of live queue pairs used to
    /// resolve a CQE's `wr_id` and advance the queue pair's tail.
    pub fn poll(&mut self, device: &Device, wc: &mut [ibv_wc]) -> Result<usize, &'static str> {
        self.poll_count = self.poll_count.wrapping_add(1);
        if self.poll_count % DRAIN_EVENTS_INTERVAL == 0 {
            let _ = uverbs(self.device_handle, DrainEvents, UserSlice::EMPTY, UserSlice::EMPTY);
        }

        let mut completions = 0;
        while completions < wc.len() {
            if self.poll_one(device, &mut wc[completions])? {
                completions += 1;
            } else {
                break;
            }
        }
        unsafe { &*self.doorbell }.update_consumer_index.set((self.consumer_index & 0xffffff).to_be());
        Ok(completions)
    }

    /// Poll this completion queue for one work completion.
    ///
    /// Return true if there are more.
    #[allow(unreachable_patterns)]
    fn poll_one(&mut self, device: &Device, wc: &mut ibv_wc) -> Result<bool, &'static str> {
        const CQE_OPCODE_ERROR: u8 = 0x1e;
        // clear the wc first
        *wc = ibv_wc::default();
        if let Some(cqe) = self.get_next_cqe_sw() {
            self.consumer_index += 1;
            // Make sure we read CQ entry contents after we've checked the
            // ownership bit.
            compiler_fence(Ordering::SeqCst);
            wc.qp_num = cqe.qp_number();
            match device.resolve_completion(cqe.qp_number(), cqe.wqe_index().into(), cqe.is_send()) {
                Some(wr_id) => wc.wr_id = wr_id,
                None => warn!("completion has invalid queue pair number {}", cqe.qp_number()),
            }
            if cqe.opcode() == CQE_OPCODE_ERROR {
                let checksum_bytes = cqe.checksum().to_be_bytes();
                let vendor_err_syndrome = checksum_bytes[0];
                let syndrome = Syndrome::from_repr(checksum_bytes[1]).ok_or("invalid error syndrome")?;
                error!(
                    "work completion error: (QPN {}, WQE index {}, vendor syndrome {}, syndrome {:?}, opcode {})",
                    cqe.qp_number(),
                    cqe.wqe_index(),
                    vendor_err_syndrome,
                    syndrome,
                    cqe.opcode(),
                );
                // A WR error (this one included, since a QP that hits any error goes to the
                // error state and flushes every WR still outstanding) is reported to the card's
                // event queue too, with the actual cause (`WqCatastrophicError`,
                // `WqInvalidRequestError`, `WqAccessViolation`, ...) rather than just the generic
                // syndrome this CQE carries. That only reaches the log once the event queue is
                // drained, which otherwise only happens every `DRAIN_EVENTS_INTERVAL` polls — far
                // too rare to catch it before a short-lived benchmark run already aborted on this
                // exact completion. Force an out-of-band drain right here instead.
                let _ = uverbs(self.device_handle, DrainEvents, UserSlice::EMPTY, UserSlice::EMPTY);
                wc.status = match syndrome {
                    Syndrome::LocalLengthError => ibv_wc_status::IBV_WC_LOC_LEN_ERR,
                    Syndrome::LocalQpOperationError => ibv_wc_status::IBV_WC_LOC_QP_OP_ERR,
                    Syndrome::LocalProtError => ibv_wc_status::IBV_WC_LOC_PROT_ERR,
                    Syndrome::WrFlushError => ibv_wc_status::IBV_WC_WR_FLUSH_ERR,
                    Syndrome::MwBindError => ibv_wc_status::IBV_WC_MW_BIND_ERR,
                    Syndrome::BadResponseError => ibv_wc_status::IBV_WC_BAD_RESP_ERR,
                    Syndrome::LocalAccessError => ibv_wc_status::IBV_WC_LOC_ACCESS_ERR,
                    Syndrome::RemoteInvalidRequestError => ibv_wc_status::IBV_WC_REM_INV_REQ_ERR,
                    Syndrome::RemoteAccessError => ibv_wc_status::IBV_WC_REM_ACCESS_ERR,
                    Syndrome::RemoteOperationError => ibv_wc_status::IBV_WC_REM_OP_ERR,
                    Syndrome::TransportRetryExceededError => ibv_wc_status::IBV_WC_RETRY_EXC_ERR,
                    Syndrome::RnrRetryExceededError => ibv_wc_status::Type::IBV_WC_RNR_RETRY_EXC_ERR,
                    Syndrome::RemoteAbortedErr => ibv_wc_status::IBV_WC_REM_ABORT_ERR,
                    _ => ibv_wc_status::Type::IBV_WC_GENERAL_ERR,
                };
                wc.vendor_err = vendor_err_syndrome.into();
                return Ok(true);
            }
            wc.status = ibv_wc_status::IBV_WC_SUCCESS;
            wc.wc_flags = ibv_wc_flags::empty();
            if cqe.is_send() {
                let opcode = QueuePairOpcode::from_repr(cqe.opcode().into()).ok_or("invalid opcode")?;
                match opcode {
                    QueuePairOpcode::RdmaWrite => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_WRITE;
                    }
                    QueuePairOpcode::RdmaWriteImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_WRITE;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                    }
                    QueuePairOpcode::Send => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                    }
                    QueuePairOpcode::SendImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                    }
                    QueuePairOpcode::SendInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                    }
                    QueuePairOpcode::RdmaRead => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_READ;
                        wc.byte_len = cqe.byte_cnt();
                    }
                    QueuePairOpcode::AtomicCs | QueuePairOpcode::MaskedAtomicCs => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_COMP_SWAP;
                        wc.byte_len = 8;
                    }
                    QueuePairOpcode::AtomicFa | QueuePairOpcode::MaskedAtomicFa => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_FETCH_ADD;
                        wc.byte_len = 8;
                    }
                    QueuePairOpcode::LocalInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_LOCAL_INV;
                    }
                    _ => {}
                }
            } else {
                let opcode = ReceiveOpcode::from_repr(cqe.opcode().into()).ok_or("invalid opcode")?;
                wc.byte_len = cqe.byte_cnt();
                match opcode {
                    ReceiveOpcode::RdmaWriteImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV_RDMA_WITH_IMM;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    }
                    ReceiveOpcode::SendInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_INV);
                        todo!("set invalidate_rkey");
                    }
                    ReceiveOpcode::Send => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                    }
                    ReceiveOpcode::SendImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    }
                }
                wc.src_qp = cqe.rqpn();
                wc.dlid_path_bits = cqe.mlpath();
                if cqe.g() {
                    wc.wc_flags.insert(ibv_wc_flags::IBV_WC_GRH);
                }
                wc.pkey_index = (cqe.immed_rss_invalid() & 0x7f).try_into().unwrap();
                wc.slid = cqe.slid();
                wc.sl = cqe.sl();
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the next element.
    fn get_next_cqe_sw(&self) -> Option<CompletionQueueEntry> {
        let index = self.consumer_index;
        let offset = usize::try_from(index & (self.num_entries - 1)).unwrap() * CQE_SIZE;
        let cqe_bytes: [u8; CQE_SIZE] = self.buffer[offset..offset + CQE_SIZE].try_into().unwrap();
        let cqe = CompletionQueueEntry::from_bytes(cqe_bytes);
        // check if it's valid
        // the ownership bit is flipping every round
        if cqe.owner() ^ ((index & self.num_entries) != 0) {
            None
        } else {
            Some(cqe)
        }
    }
}

// PRM: "CQ DoorBell Records are aligned on an 8B boundary."
#[repr(C, align(8))]
struct CompletionQueueDoorbell {
    update_consumer_index: WriteOnly<u32>,
    arm_consumer_index: WriteOnly<u32>,
}

// CQE size is 32. There is 64 B support also available in CX3.
#[bitfield(bytes = 32)]
#[derive(Debug)]
struct CompletionQueueEntry {
    #[skip]
    __: u8,
    qp_number: B24,
    immed_rss_invalid: u32,
    g: bool,
    mlpath: B7,
    rqpn: B24,
    sl: B4,
    #[skip]
    vid: B12,
    slid: u16,
    #[skip]
    __: u32,
    byte_cnt: u32,
    wqe_index: u16,
    /// vendor_err_syndrome (u8) and syndrome (u8) on error
    checksum: u16,
    #[skip]
    __: B24,
    owner: bool,
    is_send: bool,
    #[skip]
    __: bool,
    opcode: B5,
}

#[repr(u8)]
#[derive(Debug, FromRepr)]
enum Syndrome {
    LocalLengthError = 0x01,
    LocalQpOperationError = 0x02,
    LocalProtError = 0x04,
    WrFlushError = 0x05,
    MwBindError = 0x06,
    BadResponseError = 0x10,
    LocalAccessError = 0x11,
    RemoteInvalidRequestError = 0x12,
    RemoteAccessError = 0x13,
    RemoteOperationError = 0x14,
    TransportRetryExceededError = 0x15,
    RnrRetryExceededError = 0x16,
    RemoteAbortedErr = 0x22,
}

#[repr(u32)]
#[derive(FromRepr)]
enum ReceiveOpcode {
    RdmaWriteImm = 0x0,
    Send = 0x1,
    SendImm = 0x2,
    SendInval = 0x3,
}
