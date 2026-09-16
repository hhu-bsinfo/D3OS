//! Userspace-owned completion queue: creation still goes through the kernel (it needs to build
//! an MTT for the CQE buffer and run the `Sw2HwCq` CMD-interface transition), but polling and
//! CQE parsing happen entirely against the mapped buffer from here on, without a syscall per
//! poll. Arming goes through the UAR page the kernel maps into this process at creation, so it
//! needs no syscall either. This mirrors the kernel's former
//! `os/kernel/src/device/mlx4/cq` `poll`/`poll_one`/`get_next_cqe_sw`/`arm`,
//! which were deleted from the kernel once this moved here.

use alloc::sync::Arc;
use core::mem::MaybeUninit;
use core::sync::atomic::{compiler_fence, AtomicU32, Ordering};
use core3::io;
use core3::io::{Error, ErrorKind};
use log::error;
use modular_bitfield_msb::{bitfield, prelude::*};
use spin::Mutex;
use mm::{mmap, MmapFlags, PAGE_SIZE};
use rdma::uverbs_uapi::{CreateCqRequest, CreateCqResponse, UserSlice};
use rdma::uverbs_uapi::UverbsCmd::{CreateCq, DestroyCq, DrainEvents};
use strum_macros::FromRepr;
use tock_registers::interfaces::Writeable;
use tock_registers::registers::WriteOnly;
use zerocopy::AsBytes;
use crate::cmd::uverbs;
use crate::cq::{WorkCompletion, WorkCompletionFlags, WorkCompletionOpcode, WorkCompletionStatus};
use crate::provider::IbvCompletionQueue;
use crate::provider::mlx4::{CqArmCmd, Mlx4Context};
use crate::provider::mlx4::queue_pair::QueuePairOpcode;

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
    context: Arc<Mlx4Context>,
    number: u32,
    num_entries: u32,
    buffer: &'static mut [u8],
    doorbell: *mut CompletionQueueDoorbell,
    arm_sequence_number: u32,
    consumer_index: Mutex<u32>,
    poll_count: AtomicU32,
}

impl IbvCompletionQueue for CompletionQueue {
    /// Get the number of this completion queue.
    fn number(&self) -> u32 {
        self.number
    }


    /// Poll this completion queue and return the number of new completions.
    ///
    /// This is used by ibv_poll_cq. `device` is the shared registry of live queue pairs used to
    /// resolve a CQE's `wr_id` and advance the queue pair's tail.
    fn poll(&self, wc: &mut [WorkCompletion]) -> io::Result<usize> {
        let poll_count = self.poll_count.fetch_add(1, Ordering::AcqRel);
        let mut consumer_index = self.consumer_index.lock();
        if (poll_count + 1) % DRAIN_EVENTS_INTERVAL == 0 {
            let _ = uverbs(self.context.device_handle().into(), DrainEvents, UserSlice::EMPTY, UserSlice::EMPTY);
        }

        let mut completions = 0;
        while completions < wc.len() {
            if self.poll_one(*consumer_index,&mut wc[completions])? {
                *consumer_index += 1;
                completions += 1;
            } else {
                break;
            }
        }
        unsafe { &*self.doorbell }.update_consumer_index.set((*consumer_index & 0xffffff).to_be());
        Ok(completions)
    }
}

impl Drop for CompletionQueue {
    fn drop(&mut self) {
        uverbs(self.context.device_handle.into(), DestroyCq, UserSlice::from_ref(&self.number), UserSlice::EMPTY)
            .expect("failed to destroy completion queue");
    }
}

impl CompletionQueue {
    pub fn create(context: Arc<Mlx4Context>, min_num_entries: i32) -> io::Result<Self> {
        let num_entries = u32::try_from(min_num_entries)
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "cq_entries must be positive"))?
            .next_power_of_two().max(1);
        let size = num_entries as usize * CQE_SIZE;
        let buffer = mmap(0, size.next_multiple_of(PAGE_SIZE), MmapFlags::empty())
            .map_err(|_| Error::new(ErrorKind::Other, "failed to allocate CQE buffer"))?;
        buffer.fill(0);

        let doorbell_ptr: *mut CompletionQueueDoorbell = mmap(0, size_of::<CompletionQueueDoorbell>(), MmapFlags::empty())
            .map_err(|_| Error::new(ErrorKind::Other, "failed to allocate CQ doorbell"))?
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
        uverbs(context.device_handle().into(), CreateCq, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp))?;
        let resp = unsafe { resp.assume_init() };

        let mut cq = Self {
            context,
            number: resp.cq_num,
            num_entries,
            buffer,
            doorbell: doorbell_ptr,
            arm_sequence_number: 1,
            consumer_index: Mutex::new(0),
            poll_count: AtomicU32::new(0),
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
        let sn = self.arm_sequence_number & 3;
        let ci = *self.consumer_index.lock() & 0xffffff;
        let cmd = CqArmCmd::ArmNext;
        unsafe { &*self.doorbell }.arm_consumer_index.set((sn << 28 | (cmd as u32) << 24 | ci).to_be());
        // Make sure that the doorbell record in host memory is
        // written before ringing the doorbell via PCI MMIO.
        compiler_fence(Ordering::SeqCst);
        self.context.ring_cq_doorbell(self.number, CqArmCmd::ArmNext, sn as u8, ci);
    }

    /// Poll this completion queue for one work completion.
    ///
    /// Return true if there are more.
    #[allow(unreachable_patterns)]
    fn poll_one(&self, index: u32, wc: &mut WorkCompletion) -> io::Result<bool> {
        const CQE_OPCODE_ERROR: u8 = 0x1e;
        // clear the wc first
        *wc = WorkCompletion::default();
        if let Some(cqe) = self.get_cqe_sw(index) {
            wc.qp_num = cqe.qp_number();
            match self.context.resolve_completion(cqe.qp_number(), cqe.wqe_index().into(), cqe.is_send()) {
                Some(wr_id) => wc.wr_id = wr_id,
                None => {
                    // Either the QP number isn't in the registry, or the wr_id/meta lookup for
                    // this WQE index failed. Either way we cannot say which work request this
                    // completion belongs to, so report it as failed rather than silently handing
                    // back a default (wr_id = 0) success completion — that would look like a
                    // completion for whatever request happens to occupy slot 0.
                    error!(
                        "completion for QP {} WQE index {} could not be resolved to a work request",
                        cqe.qp_number(),
                        cqe.wqe_index(),
                    );
                    wc.status = WorkCompletionStatus::GeneralError;
                    return Ok(true);
                }
            }
            if cqe.opcode() == CQE_OPCODE_ERROR {
                let checksum_bytes = cqe.checksum().to_be_bytes();
                wc.vendor_err = checksum_bytes[0].into();
                wc.status = parse_syndrome(checksum_bytes[1]);
                error!(
                    "work completion error: (QPN {}, WQE index {}, vendor syndrome {}, syndrome {:?}, opcode {})",
                    cqe.qp_number(),
                    cqe.wqe_index(),
                    wc.vendor_err,
                    wc.status,
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
                let _ = uverbs(self.context.device_handle.into(), DrainEvents, UserSlice::EMPTY, UserSlice::EMPTY);
                return Ok(true);
            }
            wc.status = WorkCompletionStatus::Success;
            wc.wc_flags = WorkCompletionFlags::empty();
            if cqe.is_send() {
                let opcode = QueuePairOpcode::from_repr(cqe.opcode().into()).ok_or(Error::new(ErrorKind::Other, "invalid opcode"))?;
                match opcode {
                    QueuePairOpcode::RdmaWrite => {
                        wc.opcode = WorkCompletionOpcode::RdmaWrite;
                    }
                    QueuePairOpcode::RdmaWriteImm => {
                        wc.opcode = WorkCompletionOpcode::RdmaWrite;
                        wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_WITH_IMM);
                    }
                    QueuePairOpcode::Send => {
                        wc.opcode = WorkCompletionOpcode::Send;
                    }
                    QueuePairOpcode::SendImm => {
                        wc.opcode = WorkCompletionOpcode::Send;
                        wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_WITH_IMM);
                    }
                    QueuePairOpcode::SendInval => {
                        wc.opcode = WorkCompletionOpcode::Send;
                    }
                    QueuePairOpcode::RdmaRead => {
                        wc.opcode = WorkCompletionOpcode::RdmaRead;
                        wc.byte_len = cqe.byte_cnt();
                    }
                    QueuePairOpcode::AtomicCs | QueuePairOpcode::MaskedAtomicCs => {
                        wc.opcode = WorkCompletionOpcode::CompareAndSwap;
                        wc.byte_len = 8;
                    }
                    QueuePairOpcode::AtomicFa | QueuePairOpcode::MaskedAtomicFa => {
                        wc.opcode = WorkCompletionOpcode::FetchAdd;
                        wc.byte_len = 8;
                    }
                    QueuePairOpcode::LocalInval => {
                        wc.opcode = WorkCompletionOpcode::LocalInvalidate;
                    }
                    _ => {}
                }
            } else {
                let opcode = ReceiveOpcode::from_repr(cqe.opcode().into()).ok_or(Error::new(ErrorKind::Other, "invalid opcode"))?;
                wc.byte_len = cqe.byte_cnt();
                match opcode {
                    ReceiveOpcode::RdmaWriteImm => {
                        wc.opcode = WorkCompletionOpcode::RecvRdmaWithImm;
                        wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    }
                    ReceiveOpcode::SendInval => {
                        wc.opcode = WorkCompletionOpcode::Recv;
                        wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_WITH_INV);
                        todo!("set invalidate_rkey");
                    }
                    ReceiveOpcode::Send => {
                        wc.opcode = WorkCompletionOpcode::Recv;
                    }
                    ReceiveOpcode::SendImm => {
                        wc.opcode = WorkCompletionOpcode::Recv;
                        wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    }
                }
                wc.src_qp = cqe.rqpn();
                wc.dlid_path_bits = cqe.mlpath();
                if cqe.g() {
                    wc.wc_flags.insert(WorkCompletionFlags::IBV_WC_GRH);
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

    /// Get the CQE at `index` if it is owned by software
    ///
    /// The HCA writes a CQE back-to-front and sets the ownership bit — the top bit of the last
    /// byte of the CQE — last, so it acts as the publication flag for the rest of the entry. We
    /// therefore read that one byte alone (volatile: the compiler must not reorder or elide this
    /// load, since nothing else touches this memory from its point of view), check ownership, and
    /// only once we've established the CQE is ours do we read the remaining body — again
    /// volatile, and after an `Acquire` fence so the body read cannot be speculated/reordered
    /// ahead of the ownership check on either the compiler or the CPU side.
    fn get_cqe_sw(&self, index: u32) -> Option<CompletionQueueEntry> {
        let base = unsafe { self.buffer.as_ptr().add((index & (self.num_entries - 1)) as usize * CQE_SIZE) };
        let mut cqe = CompletionQueueEntry::new();

        // Probe just the ownership before reading rest of CQE
        cqe.bytes[CQE_SIZE-1] = unsafe { core::ptr::read_volatile(base.add(CQE_SIZE-1)) };

        // the ownership bit flips every round
        if cqe.owner() != ((index / self.num_entries) & 1) as u8 {
            return None;
        }

        // Make sure we read the rest of the CQE contents only after we've checked the ownership
        // bit that the HCA writes last.
        core::sync::atomic::fence(Ordering::Acquire);
        for (i, byte) in cqe.bytes.iter_mut().enumerate() {
            *byte = unsafe { core::ptr::read_volatile(base.add(i)) };
        }
        Some(cqe)
    }
}

// PRM: "CQ DoorBell Records are aligned on an 8B boundary."
// The specifies the completion queue doorbell record defined in PRM 16.2
// It should not be confused with the cq doorbell register in the UAR page.
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
    checksum: B16,
    #[skip]
    __: B24,
    owner: B1,
    is_send: bool,
    #[skip]
    __: bool,
    opcode: B5,
}

/// parse ConnectX-3 specific Work Completion syndromes
fn parse_syndrome(syndrome: u8) -> WorkCompletionStatus {
    match syndrome {
        0x01 => WorkCompletionStatus::LocalLengthError,
        0x02 => WorkCompletionStatus::LocalQpOperationError,
        0x04 => WorkCompletionStatus::LocalProtError,
        0x05 => WorkCompletionStatus::WrFlushError,
        0x06 => WorkCompletionStatus::MwBindError,
        0x10 => WorkCompletionStatus::BadResponseError,
        0x11 => WorkCompletionStatus::LocalAccessError,
        0x12 => WorkCompletionStatus::RemoteInvalidRequestError,
        0x13 => WorkCompletionStatus::RemoteAccessError,
        0x14 => WorkCompletionStatus::RemoteOperationError,
        0x15 => WorkCompletionStatus::TransportRetryExceededError,
        0x16 => WorkCompletionStatus::RnrRetryExceededError,
        0x22 => WorkCompletionStatus::RemoteAbortedErr,
        _ => WorkCompletionStatus::GeneralError
    }
}

#[repr(u32)]
#[derive(FromRepr)]
enum ReceiveOpcode {
    RdmaWriteImm = 0x0,
    Send = 0x1,
    SendImm = 0x2,
    SendInval = 0x3,
}
