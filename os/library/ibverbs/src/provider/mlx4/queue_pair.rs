//! Userspace-owned queue pair: creation still goes through the kernel (it needs to build an MTT
//! for the WQE buffer, map a UAR/BlueFlame page pair, and run the CMD-interface state
//! transitions), but posting and WQE bookkeeping happen entirely against the mapped buffer from
//! here on, without a syscall per post. This mirrors the kernel's former
//! `os/kernel/src/device/mlx4/queue_pair.rs` `post_send`/`post_receive`/`check_wqe_index`/etc,
//! which were deleted from the kernel once this moved here.

use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;
use core::mem::MaybeUninit;
use core::sync::atomic::{compiler_fence, AtomicBool, Ordering};
use bitflags::bitflags;
use core3::io;
use core3::io::{Error, ErrorKind};
use tock_registers::interfaces::Writeable;
use tock_registers::{register_bitfields, register_structs};
use tock_registers::registers::WriteOnly;
use zerocopy::{BigEndian, FromBytes, U16, U32, U64};
use log::error;
use spin::RwLock;
use mm::{mmap, MmapFlags, PAGE_SIZE};
use rdma::ib_core::{ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type, ibv_send_flags, ibv_send_wr_wr, ibv_sge, ibv_wr_opcode};
use rdma::uverbs_uapi::{CreateQpRequest, CreateQpResponse, ModifyQpRequest, UserSlice};
use rdma::uverbs_uapi::UverbsCmd::{CreateQp, DestroyQp, ModifyQp};
use strum_macros::FromRepr;
use crate::cmd::uverbs;
use crate::provider::{IbvQueuePair, QpInitAttr, ReceiveWorkRequest, SendWorkRequest};
use super::Mlx4Context;

pub(crate) struct QueuePair {
    context: Arc<Mlx4Context>,
    qp_type: ibv_qp_type::Type,
    pub(crate) number: u32,
    state: RwLock<ibv_qp_state>,
    rq: RwLock<WorkQueue>,
    sq: RwLock<WorkQueue>,
    receive_wqe_counter: *mut ReceiveWQECounter,
    doorbell_page: *mut DoorbellPage,
    // TODO: not used yet, see the dead `if false && num_req == 1` BlueFlame branch in post_send.
    #[allow(dead_code)]
    blueflame_page: *mut u8,
}

impl IbvQueuePair for QueuePair {
    fn number(&self) -> u32 {
        self.number
    }

    /// Post a work request to receive data.
    ///
    /// This is used by ibv_post_recv.
    unsafe fn post_receive(&self, wrs: &[ReceiveWorkRequest]) -> io::Result<()> {
        let state = *self.state.read();
        if state != ibv_qp_state::IBV_QPS_RTR && state != ibv_qp_state::IBV_QPS_RTS {
            return Err(Error::new(ErrorKind::Other, "queue pair cannot receive in this state"));
        }
        let mut rq = self.rq.write();
        let mut num_req = 0;
        for curr in wrs {
            // make sure that we're not overflowing
            if rq.would_overflow() {
                return Err(Error::new(ErrorKind::Other, "receive queue would overflow"));
            }
            // check that this work request is not too big
            if curr.sges.len() as u32 > rq.max_gs {
                return Err(Error::new(ErrorKind::Other, "work request has too many sges"));
            }
            let mut sge_index = 0;
            for sge in &curr.sges {
                let elem = rq.get_data_segment(rq.head, sge_index).unwrap();
                elem.copy_from_sge(sge);
                sge_index += 1;
            }

            // Write the wr id and the chain size, so that the completion queue can recover them.
            // Every receive WQE produces its own completion, so a chain is always a single WQE
            // long — but it still has to be recorded: `poll_one` advances the receive queue's
            // tail by this value, so leaving it at zero means the tail never moves and the queue
            // reports an overflow after `max_post` posts no matter how many completed.
            rq.update_meta_for_head(curr.wr_id, 1);

            // Terminate the scatter list, but only if this work request left a segment of the
            // WQE unused — a full one needs no terminator, and writing one would spill into the
            // next WQE and invalidate a receive buffer that is still (or about to be) posted.
            if sge_index < rq.max_gs {
                let last_elem = rq.get_data_segment(rq.head, sge_index).unwrap();
                *last_elem = WqeDataSegment::last();
            }
            rq.head = rq.head.wrapping_add(1);
            num_req += 1;
        }

        // return if we don't have anything to do
        if num_req == 0 {
            return Ok(());
        }
        // make sure that the descriptors are written before the doorbell
        compiler_fence(Ordering::SeqCst);
        unsafe { &*self.receive_wqe_counter }.set(rq.head & 0xffff);
        Ok(())
    }

    /// Post a work request to send data.
    ///
    /// This is used by ibv_post_send.
    unsafe fn post_send(&self, wrs: &[SendWorkRequest]) -> io::Result<()> {
        if *self.state.read() != ibv_qp_state::IBV_QPS_RTS {
            return Err(Error::new(ErrorKind::Other, "queue pair cannot send in this state"));
        }
        // TODO: the Nautilus driver uses sq.next_wqe
        let mut sq = self.sq.write();
        let mut num_req = 0;
        let mut chain_size = 1;

        let mut peekable = wrs.iter().peekable();
        while let Some(curr) = peekable.next() {
            // make sure that we're not overflowing
            if sq.would_overflow() {
                return Err(Error::new(ErrorKind::Other, "send queue would overflow"));
            }
            // check that this work request is not too big
            if u32::try_from(curr.sges.len()).unwrap() > sq.max_gs {
                return Err(Error::new(ErrorKind::Other, "work request has too many sges"));
            }


            let mut wqe_offset: usize = sq.wqe_byte_offset(sq.head);
            let control_segment_offset = wqe_offset;
            // TODO: check for buffer overflow
            let ctrl: &mut WqeControlSegment = sq.get_in_buffer(control_segment_offset).unwrap();
            ctrl.vlan_cv_f_ds = 0.into();
            let wqe_segment_flags: WqeControlSegmentFlags = curr.send_flags.into();
            ctrl.flags = wqe_segment_flags.bits().into();
            //ctrl.flags = WqeControlSegmentFlags::CQ_UPDATE.bits().into();
            ctrl.flags2 = 0.into();

            wqe_offset += size_of::<WqeControlSegment>();
            let mut wqe_size = size_of::<WqeControlSegment>();
            match self.qp_type {
                ibv_qp_type::IBV_QPT_RC | ibv_qp_type::IBV_QPT_UC => {
                    // extra segments are only required for RDMA
                    if curr.opcode == ibv_wr_opcode::IBV_WR_RDMA_READ || curr.opcode == ibv_wr_opcode::IBV_WR_RDMA_WRITE {
                        let wqe: &mut WqeRemoteAddressSegment = sq.get_in_buffer(wqe_offset).unwrap();
                        *wqe = WqeRemoteAddressSegment::from_wr(&curr.wr)?;
                        wqe_offset += size_of::<WqeRemoteAddressSegment>();
                        wqe_size += size_of::<WqeRemoteAddressSegment>();
                    }
                }
                ibv_qp_type::IBV_QPT_UD => {
                    let wqe: &mut WqeDatagramSegment = sq.get_in_buffer(wqe_offset).unwrap();
                    *wqe = WqeDatagramSegment::from_wr(&curr.wr)?;
                    wqe_offset += size_of::<WqeDatagramSegment>();
                    wqe_size += size_of::<WqeDatagramSegment>();
                }
                #[allow(unreachable_patterns)]
                _ => return Err(Error::new(ErrorKind::Other, "invalid queue pair type")),
            }

            // Write data segments in reverse order, so as to overwrite
            // cacheline stamp last within each cacheline. This avoids issues
            // with WQE prefetching.
            wqe_offset += (usize::try_from(curr.sges.len()).unwrap() - 1) * size_of::<WqeDataSegment>();
            for sge in curr.sges.iter().rev() {
                let elem: &mut WqeDataSegment = sq.get_in_buffer(wqe_offset).unwrap();
                elem.copy_from_sge(sge);
                wqe_offset -= size_of::<WqeDataSegment>();
                wqe_size += size_of::<WqeDataSegment>();
            }

            // Possibly overwrite stamping in cacheline with LSO segment
            // only after making sure all data segments are written.
            compiler_fence(Ordering::SeqCst);
            let ctrl: &mut WqeControlSegment = sq.get_in_buffer(control_segment_offset).unwrap();
            ctrl.vlan_cv_f_ds = u32::try_from(wqe_size / 16).unwrap().into();
            // Make sure descriptor is fully written before setting ownership
            // bit (because HW can start executing as soon as we do).
            compiler_fence(Ordering::SeqCst);
            // TODO: opcode check
            let opcode = match curr.opcode {
                ibv_wr_opcode::IBV_WR_RDMA_WRITE => QueuePairOpcode::RdmaWrite,
                ibv_wr_opcode::IBV_WR_SEND => QueuePairOpcode::Send,
                ibv_wr_opcode::IBV_WR_RDMA_READ => QueuePairOpcode::RdmaRead,
            } as u32;
            let owner = match sq.head & sq.wqe_cnt {
                0 => 0,
                _ => 1 << 31,
            };
            ctrl.owner_opcode = (owner | opcode).into();
            // We can improve latency by not stamping the last send queue WQE
            // until after ringing the doorbell, so only stamp here if there are
            // still more WQEs to post.
            let end_of_headroom = sq.head + sq.spare_wqes.unwrap();
            sq.stamp_wqe(end_of_headroom, wqe_size)?;

            if curr.send_flags.contains(ibv_send_flags::SIGNALED) {
                // write wr id, so that completion queue poll can recover it
                sq.update_meta_for_head(curr.wr_id, chain_size);

                chain_size = 1;
            } else {
                chain_size += 1;
            }

            num_req += 1;
            sq.head = sq.head.wrapping_add(1);
            // TODO: support multiple work requests ; Done
        }
        // return if we don't have anything to do
        if num_req == 0 {
            return Ok(());
        }
        // TODO: bf fails for RDMA writes
        if false && num_req == 1 {
            // TODO: why decrement index and not just wqe_byte_offset(index - 1)
            let index = sq.head - 1;
            let ctrl_offset = sq.wqe_byte_offset(index);
            let ctrl: &mut WqeControlSegment = sq.get_in_buffer(ctrl_offset).unwrap();
            // Make sure that descriptor is written to memory
            // before writing to BlueFlame page.
            compiler_fence(Ordering::SeqCst);
            // the UAR determines which BlueFlame page we can use
            // we just use the first register (0..bf_reg_size)
            // each register consists of two buffers (bf_reg_size/2)
            // which we have to alternate between
            /* TODO: assign BlueFlame page. A QP can only use a BlueFlame page with the index equal to the QP UAR.
            let bf_reg: &mut [u64] = blueflame.as_slice_mut((index as usize % 2) * (caps.bf_reg_size() / 2), caps.bf_reg_size() / 8)?;
            let src = self.sq.buffer.as_ptr() as *const u64;
            let size = ctrl.size() / 8;
            unsafe { copy_nonoverlapping(src, bf_reg.as_ptr(), size) };
             */
            // TODO: will this work when mixing BF and normal sends?
        } else {
            // Make sure that descriptors are written before doorbell.
            compiler_fence(Ordering::SeqCst);
            unsafe { &*self.doorbell_page }.send_queue_number.set((self.number << 8).to_be());
        }
        Ok(())
    }

    fn modify(&self, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask) -> io::Result<()> {
        let attr = *attr;

        let req = ModifyQpRequest {
            qp_num: self.number,
            attr,
            attr_mask
        };

        let mut state = self.state.write();
        uverbs(self.context.device_handle, ModifyQp, UserSlice::from_ref(&req), UserSlice::EMPTY)?;

        if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_STATE) {
            *state = attr.qp_state;
        }

        Ok(())
    }
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        let qp_num = self.number();
        uverbs(self.context.device_handle(), DestroyQp, UserSlice::from_ref(&qp_num), UserSlice::EMPTY)
            .expect("failed to destroy queue pair");
    }
}

impl QueuePair {
    pub(crate) fn create(context: Arc<Mlx4Context>, attr: &QpInitAttr) -> io::Result<QueuePair> {
        let mut rq = WorkQueue::new_receive_queue(&context, &attr.cap)?;
        let mut sq = WorkQueue::new_send_queue(&context, &attr.cap, attr.qp_type)?;

        let buf_size: usize = (rq.size() + sq.size()).try_into().unwrap();
        let buffer = mmap(0, buf_size.next_multiple_of(PAGE_SIZE), MmapFlags::empty()).expect("failed to allocate buffer");
        buffer.fill(0);
        let buffer_ptr = buffer.as_mut_ptr();

        // NOTE: ibverbs does not allocate a complete page for one Doorbell, instead it uses a shared allocator
        //       in the device context for all Doorbells. This Allocator only allocates pages if it ran out of memory
        let receive_wqe_counter_ptr: *mut ReceiveWQECounter = mmap(0, size_of::<ReceiveWQECounter>(), MmapFlags::empty()).expect("failed to allocate doorbell page").as_mut_ptr().cast();
        let receive_wqe_counter = unsafe { &*receive_wqe_counter_ptr };
        receive_wqe_counter.set(0);

        if rq.wqe_shift > sq.wqe_shift {
            // RQ first
            let (rq_buf, sq_buf) = buffer.split_at_mut(rq.size().try_into().unwrap());
            rq.buffer = Some(rq_buf);
            sq.buffer = Some(sq_buf);
        } else {
            // SQ first
            let (sq_buf, rq_buf) = buffer.split_at_mut(sq.size().try_into().unwrap());
            rq.buffer = Some(rq_buf);
            sq.buffer = Some(sq_buf);
        }

        // Before passing the QP to the HW, make sure the ownership bits of the send queue are
        // set and the SQ headroom is stamped so the hardware doesn't start processing stale work
        // requests. This used to run in the kernel right before the RST->INIT transition; now
        // that the buffer is userspace-owned, it has to happen here instead, before `CreateQp`
        // is even issued (the kernel does the RST->INIT transition as part of that call).
        for i in 0..sq.wqe_cnt {
            let ctrl: &mut WqeControlSegment = sq.get_in_buffer(sq.wqe_byte_offset(i)).ok_or(Error::new(ErrorKind::Other, "invalid send queue offset"))?;
            ctrl.owner_opcode = (1u32 << 31).into();
            ctrl.vlan_cv_f_ds = (1u32 << (sq.wqe_shift - 4)).into();
            sq.stamp_wqebb(i)?;
        }

        let req = CreateQpRequest {
            _pd_handle: 0,
            qp_type: attr.qp_type,
            send_cq_num: attr.send_cq.number(),
            recv_cq_num: attr.recv_cq.number(),
            _sq_sig_all: 0,
            _reserved: 0,
            buffer: buffer_ptr,
            doorbell_ptr: receive_wqe_counter_ptr.cast(),
            log_sq_bb_count: sq.wqe_cnt.ilog2().try_into().unwrap(),
            log_sq_stride: sq.wqe_shift.try_into().unwrap(),
            inline_recv_size: 0,
            log_rq_wqe_count: rq.wqe_cnt.ilog2().try_into().unwrap(),
            log_rq_stride: rq.wqe_shift.try_into().unwrap(),
        };

        let mut resp = MaybeUninit::<CreateQpResponse>::uninit();
        uverbs(context.device_handle, CreateQp, UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp))?;
        let resp = unsafe { resp.assume_init() };

        Ok(QueuePair {
            context,
            qp_type: attr.qp_type,
            number: resp.qp_num,
            state: RwLock::new(ibv_qp_state::IBV_QPS_RESET),
            rq: RwLock::new(rq),
            sq: RwLock::new(sq),
            receive_wqe_counter: receive_wqe_counter_ptr,
            doorbell_page: resp.doorbell_page.cast(),
            blueflame_page: resp.blueflame_page,
        })
    }


    /// Advance the tail of the receive queue.
    ///
    /// This is called on work completion.
    #[inline(always)]
    pub(super) fn advance_receive_queue_by(&mut self, by: u32) {
        self.rq.write().tail += by;
    }

    pub fn resolve_completion(&self, wqe_index: u32, is_send: bool) -> Option<u64> {
        if is_send {
            self.sq.write().resolve_completion(self.number, wqe_index)
        } else {
            self.rq.write().resolve_completion(self.number, wqe_index)
        }
    }
}

#[repr(transparent)]
struct ReceiveWQECounter(WriteOnly<u32>);

impl Writeable for ReceiveWQECounter {
    type T = u32;
    type R = ();

    #[inline]
    fn set(&self, value: Self::T) {
        self.0.set(value.to_be())
    }
}

#[repr(u32)]
#[derive(FromRepr)]
pub(crate) enum QueuePairOpcode {
    Nop = 0x00,
    SendInval = 0x01,
    RdmaWrite = 0x08,
    RdmaWriteImm = 0x09,
    Send = 0x0a,
    SendImm = 0x0b,
    Lso = 0x0e,
    RdmaRead = 0x10,
    AtomicCs = 0x11,
    AtomicFa = 0x12,
    MaskedAtomicCs = 0x14,
    MaskedAtomicFa = 0x15,
    BindMw = 0x18,
    Fmr = 0x19,
    LocalInval = 0x1b,
    ConfigCmd = 0x1f,
}

// TODO: define DoorbellEq and DoorbellPage to register_structs!

register_bitfields![u32,
    pub SendQueueNumber [
        NUM OFFSET(8) NUMBITS(24)
    ],
    pub CpSnCmdNum [
        CPN OFFSET(0)  NUMBITS(24),
        CMD OFFSET(24) NUMBITS(3),
        SN  OFFSET(28) NUMBITS(2)
    ],
    pub CpConsumerIndex [
        CP_CI OFFSET(0) NUMBITS(24),
    ],
    pub DoorbellEqField [
        CI OFFSET(0)  NUMBITS(24),
        A  OFFSET(31) NUMBITS(1)
    ]
];

pub struct DoorbellEq  {
    pub val: WriteOnly<u32, DoorbellEqField::Register>,
    _reserved1: u32
}

register_structs! {
    pub DoorbellPage {
    (0x000 => _reserved1),
    (0x014 => pub send_queue_number: WriteOnly<u32, SendQueueNumber::Register>),
    (0x018 => _reserved2),

    // CQ
    /// contains the sequence number, the command and the cq number
    (0x020 => pub cq_sn_cmd_num: WriteOnly<u32, CpSnCmdNum::Register>),
    (0x024 => pub cq_consumer_index: WriteOnly<u32, CpConsumerIndex::Register>),

    // skip 502 u32
    (0x028 => _padding4),

    // EQ
    // for the EQ number n the relevant doorbell is in
    // DoorbellPage (n / 4) and eq (n % 4)
    (0x800 => pub eqs: [DoorbellEq; 4]),

    // skip 503 u32
    (0x820 => _padding9),
    (0x1000 => @END),
    }
}

#[derive(FromBytes)]
#[repr(C)]
struct WqeDataSegment {
    byte_count: U32<BigEndian>,
    lkey: U32<BigEndian>,
    addr: U64<BigEndian>,
}

impl WqeDataSegment {
    /// Copy information from an sge.
    fn copy_from_sge(&mut self, sge: &ibv_sge) {
        // The address stays virtual: the lkey names a memory region whose MPT
        // start address is virtual as well, and the card resolves the address
        // through that region's MTT. (Translating to a physical address here
        // would only be right for the reserved lkey, which bypasses the MPT.)
        self.lkey.set(sge.lkey);
        self.addr.set(sge.addr);
        // sending needs a barrier here before writing the byte_count
        // field to make sure that all the data is visible before the
        // byte_count field is set. Otherwise, if the segment begins a new
        // cacheline, the HCA prefetcher could grab the 64-byte chunk and
        // get a valid (!= * 0xffffffff) byte count but stale data, and end
        // up sending the wrong data.
        compiler_fence(Ordering::SeqCst);
        self.byte_count.set(sge.length);
    }

    /// Create a dummy element to be the last in the queue.
    fn last() -> WqeDataSegment {
        const INVALID_LKEY: u32 = 0x100;
        Self {
            byte_count: 0.into(),
            lkey: INVALID_LKEY.into(),
            addr: 0.into(),
        }
    }
}

bitflags! {
    struct WqeControlSegmentFlags: u32 {
        const NEC = 1 << 29;
        const IIP = 1 << 28;
        const ILP = 1 << 27;
        const FENCE = 1 << 6;
        const CQ_UPDATE = 3 << 2;
        const SOLICITED = 1 << 1;
        const IP_CSUM = 1 << 4;
        const TCP_UDP_CSUM = 1 << 5;
        const INS_CVLAN = 1 << 6;
        const INS_SVLAN = 1 << 7;
        const STRONG_ORDER = 1 << 7;
        const FORCE_LOOPBACK = 1 << 0;
    }
}

impl From<ibv_send_flags> for WqeControlSegmentFlags {
    fn from(flags: ibv_send_flags) -> Self {
        let mut out = WqeControlSegmentFlags::empty();

        if flags.contains(ibv_send_flags::FENCE) {
            out |= WqeControlSegmentFlags::FENCE;
        }
        if flags.contains(ibv_send_flags::SOLICITED) {
            out |= WqeControlSegmentFlags::SOLICITED;
        }
        // CQ update for signaled WRs
        if flags.contains(ibv_send_flags::SIGNALED) {
            out |= WqeControlSegmentFlags::CQ_UPDATE;
        }
        out
    }
}

#[derive(FromBytes)]
#[repr(C)]
struct WqeRemoteAddressSegment {
    va: U64<BigEndian>,
    key: U32<BigEndian>,
    rsvd: u32,
}

impl WqeRemoteAddressSegment {
    /// Create a remote address segment from a wr wr.
    fn from_wr(wr: &ibv_send_wr_wr) -> io::Result<Self> {
        if let ibv_send_wr_wr::rdma { remote_addr, rkey } = wr {
            Ok(Self {
                va: (*remote_addr).into(),
                key: (*rkey).into(),
                rsvd: 0,
            })
        } else {
            Err(Error::new(ErrorKind::InvalidData, "invalid wr field"))
        }
    }
}

#[derive(FromBytes)]
#[repr(C)]
struct WqeControlSegment {
    owner_opcode: U32<BigEndian>,
    /// DS: WQE size in octowords (16-byte units)
    vlan_cv_f_ds: U32<BigEndian>,
    flags: U32<BigEndian>,
    flags2: U32<BigEndian>,
}

impl WqeControlSegment {
    /// Size of the WQE in bytes
    fn size(&self) -> u32 {
        (self.vlan_cv_f_ds.get() & 0x3f) << 4
    }
}

#[derive(FromBytes)]
#[repr(C)]
struct WqeDatagramSegmentAv {
    port_pd: U32<BigEndian>,
    _reserved1: u8,
    g_slid: u8,
    dlid: U16<BigEndian>,
    _reserved2: u8,
    gid_index: u8,
    stat_rate: u8,
    hop_limit: u8,
    sl_tclass_flowlabel: u32,
    dgid: [u32; 4],
}


const ETH_ALEN: usize = 6;

#[derive(FromBytes)]
#[repr(C)]
struct WqeDatagramSegment {
    av: WqeDatagramSegmentAv,
    dst_qpn: U32<BigEndian>,
    qkey: U32<BigEndian>,
    vlan: u16,
    mac: [u8; ETH_ALEN],
}

impl WqeDatagramSegment {
    /// Create a datagram segment from a wr wr.
    fn from_wr(wr: &ibv_send_wr_wr) -> io::Result<Self> {
        if let ibv_send_wr_wr::ud { ah, remote_qpn, remote_qkey } = wr {
            Ok(Self {
                av: WqeDatagramSegmentAv {
                    port_pd: (ah.port << 24).into(),
                    _reserved1: 0,
                    g_slid: ah.slid & 0x7f,
                    dlid: ah.dlid.into(),
                    _reserved2: 0,
                    gid_index: 0,
                    stat_rate: 0,
                    hop_limit: 0,
                    sl_tclass_flowlabel: 0,
                    dgid: [0; 4],
                },
                dst_qpn: (*remote_qpn).into(),
                qkey: (*remote_qkey).into(),
                vlan: 0,
                mac: [0; ETH_ALEN],
            })
        } else {
            Err(Error::new(ErrorKind::InvalidData, "invalid wr field"))
        }
    }
}


// TODO: why not use a struct instead of a tuple for WorkQueueMeta
type WorkQueueMeta<U, T> = (U, T);

#[derive(Debug)]
enum WorkQueueType {
    Send,
    Receive,
}

#[derive(Debug)]
struct WorkQueue {
    wq_type: WorkQueueType,
    wqe_cnt: u32,
    max_post: u32,
    max_gs: u32,
    buffer: Option<&'static mut [u8]>,
    wqe_shift: u32,
    spare_wqes: Option<u32>,
    head: u32,
    tail: u32,
    meta: Vec<WorkQueueMeta<u64, u32>>,
    /// Set once this queue's tail has been seen to disagree with the card, see
    /// [`QueuePair::check_wqe_index`]. Only the first disagreement is worth logging.
    divergence_reported: bool,
}

const IB_SQ_MIN_WQE_SHIFT: u32 = 6;
const IB_MAX_HEADROOM: u32 = 2048;
const IB_SQ_MAX_SPARE: u32 = ib_sq_headroom(IB_SQ_MIN_WQE_SHIFT);

const fn ib_sq_headroom(shift: u32) -> u32 {
    (IB_MAX_HEADROOM >> shift) + 1
}

impl WorkQueue {
    /// Compute the size of the receive queue and return it.
    fn new_receive_queue(device: &Mlx4Context, ib_caps: &ibv_qp_cap) -> io::Result<Self> {
        // check the RQ size before proceeding
        if ib_caps.max_recv_wr > ((1 << device.log_max_qp_size) - IB_SQ_MAX_SPARE)
            || ib_caps.max_recv_sge > 1 << device.log_max_rq_sge
        {
            return Err(Error::new(ErrorKind::InvalidInput, "RQ size is invalid"));
        }
        let mut wqe_cnt = ib_caps.max_recv_wr;
        if wqe_cnt < 256 {
            wqe_cnt = 256;
        }
        wqe_cnt = wqe_cnt.next_power_of_two();
        let mut max_gs = ib_caps.max_recv_sge;
        if max_gs < 1 {
            max_gs = 1;
        }
        max_gs = max_gs.next_power_of_two();
        let wqe_shift = (max_gs * u32::try_from(size_of::<WqeDataSegment>()).unwrap()).ilog2();
        let mut max_post = (1 << device.log_max_qp_size) - IB_SQ_MAX_SPARE;
        if max_post > wqe_cnt {
            max_post = wqe_cnt;
        }
        Ok(Self {
            wq_type: WorkQueueType::Receive,
            wqe_cnt,
            max_post,
            max_gs,
            buffer: None,
            wqe_shift,
            spare_wqes: None,
            head: 0,
            tail: 0,
            meta: vec![(0u64, 0u32); wqe_cnt as usize],
            divergence_reported: false,
        })
    }

    /// Compute the size of the receive queue and return it.
    fn new_send_queue(device: &Mlx4Context, ib_caps: &ibv_qp_cap, qp_type: ibv_qp_type::Type) -> io::Result<Self> {
        // check the SQ size before proceeding
        if ib_caps.max_send_wr > ((1 << device.log_max_qp_size) - IB_SQ_MAX_SPARE)
            || ib_caps.max_send_sge > 1 << device.log_max_sq_sge
        {
            return Err(Error::new(ErrorKind::InvalidInput, "SQ size is invalid"));
        }
        let size: u16 = (ib_caps.max_send_sge * u32::try_from(size_of::<WqeDataSegment>()).unwrap() + send_wqe_overhead(qp_type)).try_into().unwrap();
        if size > device.max_wqe_sq_size {
            return Err(Error::new(ErrorKind::InvalidInput, "SQ size is invalid"));
        }
        let wqe_shift = size.next_power_of_two().ilog2();
        // We need to leave 2 KB + 1 WR of headroom in the SQ to allow HW to prefetch.
        let spare_wqes = ib_sq_headroom(wqe_shift);
        let mut wqe_cnt = ib_caps.max_send_wr;
        if wqe_cnt < 256 {
            wqe_cnt = 256;
        }
        wqe_cnt = (wqe_cnt + spare_wqes).next_power_of_two();
        let max_gs: u32 = (u32::from(device.max_wqe_sq_size.min((1u32 << wqe_shift).try_into().unwrap())) - send_wqe_overhead(qp_type)) / u32::try_from(size_of::<WqeDataSegment>()).unwrap();
        let max_post = wqe_cnt - spare_wqes;
        Ok(Self {
            wq_type: WorkQueueType::Send,
            wqe_cnt,
            max_post,
            max_gs,
            buffer: None,
            wqe_shift,
            spare_wqes: Some(spare_wqes),
            head: 0,
            tail: 0,
            meta: vec![(0u64, 0u32); wqe_cnt as usize],
            divergence_reported: false,
        })
    }

    /// Get the size.
    fn size(&self) -> u32 {
        self.wqe_cnt << self.wqe_shift
    }

    #[inline(always)]
    fn update_meta_for_head(&mut self, wr_id: u64, chain_size: u32) {
        let idx = self.head & (self.wqe_cnt - 1);
        self.meta.insert(idx as usize, (wr_id, chain_size));
    }

    fn get_in_buffer<T: FromBytes>(&self, byte_offset: usize) -> Option<&mut T> {
        let buffer = self.buffer.as_ref()?;
        if buffer.len() < byte_offset + size_of::<T>() {
            return None;
        }
        Some(unsafe { &mut *(buffer.as_ptr().add(byte_offset) as *mut T) })
    }

    fn wqe_byte_offset(&self, index: u32) -> usize {
        ((index & self.wqe_cnt - 1) << self.wqe_shift) as usize
    }

    /// Get the `sge_index`th data segment of the WQE at `index`.
    ///
    /// A receive WQE holds `max_gs` data segments, so unlike [`Self::get_element`] this
    /// addresses within a single WQE — adding the segment index to the WQE index would land in
    /// the following WQE instead.
    fn get_data_segment(
        &self, index: u32, sge_index: u32,
    ) -> Option<&mut WqeDataSegment> {
        let offset = self.wqe_byte_offset(index) + sge_index as usize * size_of::<WqeDataSegment>();
        self.get_in_buffer::<WqeDataSegment>(offset)
    }

    fn stamp_wqe(&mut self, index: u32, size: usize) -> io::Result<()> {
        // RPM: 10.2.1.1 SQ Headroom Invalidation
        // TODO: When the SW uses a WQE size smaller than or equal to the WQEBB, it is possible to do the following optimizations:
        // - The SW can skip the initialization of the first 64 bytes of each WQE
        // - For the invalid WQE indication, the SW can use 0xFFFFFFFF for every 64 byte block following the first, except for the first 64 bytes of each WQE

        let wqebb_count = size.div_ceil(1 << self.wqe_shift) as u32;
        for i in 0..wqebb_count {
            self.stamp_wqebb(index + i)?
        }
        Ok(())
    }

    /// Stamp this WQEBB so that it is invalid if prefetched by marking the
    /// first four bytes of every 64 byte chunk with 0xffffffff or 0x7fffffff
    /// debending on the index, it flips every wqe_cnt
    fn stamp_wqebb(&mut self, index: u32) -> io::Result<()> {
        let invalid_owner_bit = (index >> self.wqe_cnt.next_power_of_two().trailing_zeros()) % 2 == 0;
        let start = self.wqe_byte_offset(index);
        let end = start + (1 << self.wqe_shift);
        let buffer = self.buffer.as_mut().ok_or(Error::new(ErrorKind::InvalidData, "queue pair has no buffer"))?;
        for i in (start..end).step_by(64) {
            buffer[i] = 0x7F | (invalid_owner_bit as u8) << 7;
            buffer[i + 1] = 0xFF;
            buffer[i + 2] = 0xFF;
            buffer[i + 3] = 0xFF;
        }
        Ok(())
    }

    /// Check if this queue would overflow when adding a work requests.
    fn would_overflow(&mut self) -> bool {
        self.head - self.tail + 1 >= self.max_post
    }

    /// Check the work queue element index the card reports against the one we expect next.
    ///
    /// The reference driver takes the card's index as the truth
    /// (`wq->tail += (u16)(wqe_ctr - (u16)wq->tail)` in `mlx4_ib_poll_one`), while this driver
    /// advances the tail by the chain size it recorded when the work request was posted. Those
    /// two agree only as long as every completion is seen exactly once. If they drift apart the
    /// queue reports an overflow while the card still has room — and on the receive side that
    /// means we stop posting receives and the peer starts seeing RNR NAKs.
    ///
    /// Only the first disagreement per queue is logged: by then everything after it is suspect,
    /// and logging goes out over the serial console, which is slow enough to cause the very
    /// stalls being investigated.
    pub(super) fn check_wqe_index(&mut self, qp_num: u32, wqe_index: u32) {
        if self.divergence_reported {
            return;
        }
        let expected = self.tail & (self.wqe_cnt - 1);
        let reported = wqe_index & (self.wqe_cnt - 1);
        if expected != reported {
            self.divergence_reported = true;
            error!(
                "QP {qp_num}: card reports a {:?} completion for WQE {reported}, driver expected {expected} (head {}, tail {})",
                self.wq_type,
                self.head,
                self.tail,
            );
        }
    }

    fn resolve_completion(&mut self, qp_num: u32, wqe_index: u32) -> Option<u64> {
        self.check_wqe_index(qp_num, wqe_index);
        let (wr_id, chain_size) = self.meta.get(wqe_index as usize)?;
        self.tail += chain_size;
        Some(*wr_id)
    }
}

fn send_wqe_overhead(qp_type: ibv_qp_type::Type) -> u32 {
    // UD WQEs must have a datagram segment.
    // RC and UC WQEs might have a remote address segment.
    // MLX WQEs need two extra inline data segments (for the UD header and space
    // for the ICRC).
    match qp_type {
        ibv_qp_type::IBV_QPT_UD => size_of::<WqeControlSegment>() + size_of::<WqeDatagramSegment>(),
        ibv_qp_type::IBV_QPT_UC => size_of::<WqeControlSegment>() + size_of::<WqeRemoteAddressSegment>(),
        ibv_qp_type::IBV_QPT_RC => {
            size_of::<WqeControlSegment>() /* + size_of::<WqeMaskedAtomicSegment>() */
                + size_of::<WqeRemoteAddressSegment>()
        }
        #[allow(unreachable_patterns)]
        _ => size_of::<WqeControlSegment>(),
    }
        .try_into()
        .unwrap()
}
