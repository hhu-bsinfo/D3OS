//! This module consists of functions that create, work with and destroy queue
//! pairs. Its functions can change the state of a QP and query and print some
//! QP infos.

use core::{
    mem::size_of,
    sync::atomic::{compiler_fence, Ordering},
};

use alloc::{vec, vec::Vec};
use bitflags::bitflags;
use byteorder::BigEndian;
use log::trace;
use modular_bitfield_msb::{
    bitfield,
    prelude::{B12, B16, B17, B19, B2, B20, B24, B3, B4, B40, B48, B5, B53, B56, B6, B7},
};
use rdma::{
    ibv_access_flags, ibv_mtu, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type, ibv_send_flags, ibv_send_wr_wr,
    ibv_sge,
};
use strum_macros::FromRepr;
use tock_registers::registers::WriteOnly;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, Size4KiB};
use zerocopy::{AsBytes, FromBytes, U16, U32, U64};
use crate::device::mlx4::cmd::{InputParam, OutputParam};
use crate::process_manager;
use super::{cmd::{CommandInterface, Opcode}, device::{uar_index_to_hw, PAGE_SHIFT}, fw::Capabilities, icm::ICM_PAGE_SHIFT, utils, ConnectX3Nic};

const IB_SQ_MIN_WQE_SHIFT: u32 = 6;
const IB_MAX_HEADROOM: u32 = 2048;
const IB_SQ_MAX_SPARE: u32 = ib_sq_headroom(IB_SQ_MIN_WQE_SHIFT);

const fn ib_sq_headroom(shift: u32) -> u32 {
    (IB_MAX_HEADROOM >> shift) + 1
}

#[derive(Debug)]
pub(super) struct QueuePair {
    number: u32,
    state: ibv_qp_state,
    qp_type: ibv_qp_type::Type,
    port_number: Option<u8>,
    // TODO: bind the lifetime to the one of the completion queues
    send_cq_number: u32,
    receive_cq_number: u32,
    uar_idx: usize,
    uar_page: Page<Size4KiB>,
    bf_page: Page<Size4KiB>,
    mtt: Option<u64>,
    /// In units of 64 bytes
    page_offset: u8,
    doorbell_address: PhysAddr,
    log_sq_bb_count: u8,
    log_sq_stride: u8,
    log_rq_wqe_count: u8,
    log_rq_stride: u8,
}

impl QueuePair {
    /// Create a new queue pair.
    ///
    /// This includes allocating the area for the buffer itself and allocating
    /// an MTT entry for the buffer. It does *not* allocate a send queue or
    /// receive queue for the work queue.
    ///
    /// This is similar to creating a completion queue or an event queue.
    pub(super) fn new(
        dev: &mut ConnectX3Nic,
        qp_type: ibv_qp_type::Type,
        send_cq_number: u32,
        receive_cq_number: u32,
        buffer: *const u8,
        doorbell_ptr: *const u32,
        log_sq_bb_count: u8,
        log_sq_stride: u8,
        log_rq_wqe_count: u8,
        log_rq_stride: u8,
    ) -> Result<Self, &'static str> {
        let number = dev.offsets.alloc_qpn().try_into().unwrap();

        let process = process_manager().read().current_process();
        // TODO: UAR is allocated a device open
        let uar_idx = dev.offsets.alloc_uar();
        let uar = dev.map_uar(uar_idx, &process, alloc::format!("uar-{uar_idx}").as_str())?;
        let bf = dev.map_bf(uar_idx, &process)?;

        let buffer_size: u64 = (1 << (log_sq_bb_count + log_sq_stride)) + (1 << (log_rq_wqe_count + log_rq_stride));
        let buffer_addr = VirtAddr::from_ptr(buffer);

        assert_eq!(buffer.addr() % 64, 0, "buffer is not aligned to 64");
        let start: Page<Size4KiB> = Page::containing_address(buffer_addr);
        let end = start + buffer_size.div_ceil(start.size());
        let mtt = Some(dev.icm_tables.memory_regions().alloc_mtt_for_pages(&dev.capabilities, Page::range(start, end))?);
        // Offset from the first page in units of 64 bytes
        let page_offset: u8 = ((buffer_addr - start.start_address()) >> 6) as u8;

        let doorbell_address = process.virtual_address_space
            .get_phys(doorbell_ptr as u64)
            .ok_or("doorbell not mapped to physical address")?;

        let qp = Self {
            number,
            state: ibv_qp_state::IBV_QPS_RESET,
            qp_type,
            port_number: None,
            send_cq_number,
            receive_cq_number,
            uar_idx,
            uar_page: uar,
            bf_page: bf,
            mtt,
            page_offset,
            doorbell_address,
            log_sq_bb_count,
            log_sq_stride,
            log_rq_wqe_count,
            log_rq_stride,
        };
        trace!("created new QP: {qp:?}");
        Ok(qp)
    }

    /// Query this queue pair.
    pub(super) fn query(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        cmd.execute_command(Opcode::QueryQp, None, InputParam::Empty, Some(self.number), OutputParam::Mailbox)?;
        let transition: &StateTransitionCommandParameter = unsafe { cmd.output_mailbox_as_ref() };
        let context = QueuePairContext::from_bytes(transition.qpc_data);
        trace!("Queue Pair Context: {context:?}");
        Ok(())
    }

    /// Modify this queue pair.
    ///
    /// This is used by ibv_modify_qp.
    pub(super) fn modify(
        &mut self, cmd: &mut CommandInterface, caps: &Capabilities, attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
    ) -> Result<(), &'static str> {
        // TODO: this discards any parameters that aren't needed for the current transition
        // TODO: perhaps query before so that we have the current state
        const _PATH_MIGRATION_STATE_ARMED: u8 = 0x0;
        const _PATH_MIGRATION_STATE_REARM: u8 = 0x1;
        const PATH_MIGRATION_STATE_MIGRATED: u8 = 0x3;
        const DEFAULT_SCHED_QUEUE: u8 = 0x83;
        // create the context
        let mut context = QueuePairContext::new();
        let mut param_mask = OptionalParameterMask::empty();

        let next_qp_state = if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_STATE) {
            Some(attr.qp_state)
        } else {
            None
        };

        // get the right state transition
        let opcode = match (self.state, next_qp_state) {
            // initialize
            (ibv_qp_state::IBV_QPS_RESET, Some(ibv_qp_state::IBV_QPS_INIT)) => {
                // save the port number for later on
                // In earlier versions of the API, the port number was required
                // to be set as part of this transition. This is no longer the
                // case as it moved into INIT2RTR, but applications may set it
                // here, so save it for later.
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PORT) {
                    self.port_number = Some(attr.port_num);
                }
                // set required fields
                context.set_service_type(match self.qp_type {
                    ibv_qp_type::IBV_QPT_RC => 0x0,
                    ibv_qp_type::IBV_QPT_UC => 0x1,
                    ibv_qp_type::IBV_QPT_UD => 0x3,
                    #[allow(unreachable_patterns)]
                    _ => return Err("invalid queue pair type"),
                });
                context.set_path_migration_state(PATH_MIGRATION_STATE_MIGRATED);
                context.set_usr_page(uar_index_to_hw(self.uar_idx).try_into().unwrap());
                // TODO: protection domain
                context.set_cqn_send(self.send_cq_number);
                // RC needs remote read
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS));
                    context.set_remote_read(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_READ));
                }
                // RC and UC need remote write
                if self.qp_type == ibv_qp_type::IBV_QPT_RC || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS));
                    context.set_remote_write(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE));
                }
                // RC needs remote atomic
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS));
                    context.set_remote_atomic(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC));
                }
                context.set_cqn_receive(self.receive_cq_number);
                // UD needs qkey
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_QKEY));
                    context.set_qkey(attr.qkey);
                }
                // TODO: RC and UD need srq
                // TODO: RC and UD need srqn
                // TODO: fre
                context.set_log_sq_size(self.log_sq_bb_count);
                context.set_log_rq_size(self.log_rq_wqe_count);
                context.set_log_sq_stride(self.log_sq_stride - 4);
                context.set_log_rq_stride(self.log_rq_stride - 4);
                // since we can't allocate protection domains,
                // allow using the reserved lkey to refer directly to physical
                // addresses
                context.set_reserved_lkey(true);
                // TODO: sq_wqe_counter, rq_wqe_counter, is
                // TODO: hs, vsd, rss for UD
                context.set_sq_no_prefetch(false);
                // TODO: page_offset, pkey_index, disable_pkey_check
                // TOODO: rss context for UD
                context.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
                context.set_mtt_base_addr(self.mtt.ok_or("queue pair has no MTT")?);
                context.set_db_record_addr(self.doorbell_address.as_u64().try_into().unwrap());

                // The send queue's ownership bits and headroom stamping must be initialized
                // before the HW takes ownership of the buffer; since userspace now owns the
                // buffer, that init happens in `mlx4::QueuePair::create` before this transition
                // is requested.

                Opcode::Rst2InitQp
            }

            // or just stay in the current state
            // We can't even set anything here.
            (ibv_qp_state::IBV_QPS_RESET, None) => return Ok(()),

            // init -> rtr
            (ibv_qp_state::IBV_QPS_INIT, Some(ibv_qp_state::IBV_QPS_RTR)) => {
                // we need the port number for this transition
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PORT) {
                    self.port_number = Some(attr.port_num);
                }

                // set required fields
                // TODO: this might have been set in an earlier call
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PATH_MTU) {
                    context.set_mtu(attr.path_mtu as u8);
                } else {
                    // default to the highest one
                    context.set_mtu(ibv_mtu::default() as u8);
                }
                context.set_msg_max(caps.log_max_msg());

                // TODO: required parameters for RC and UC: next_recv_psn, qos_vport, roce_mode,
                if self.qp_type == ibv_qp_type::IBV_QPT_RC || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_DEST_QPN));
                    context.set_remote_qpn(attr.dest_qp_num);
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_AV));
                    context.set_primary_rlid(attr.ah_attr.dlid);
                }

                // TODO: required parameters for RC: ric
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC));
                    // TODO: check if the devices supports that many outstanding read/atomic operations
                    context.set_rra_max_checked(attr.max_dest_rd_atomic.next_power_of_two().ilog2() as u8).map_err(|_| "rra_max out of bounds")?;
                }

                // TODO: required parameters for all types: rate_limit_index
                context.set_primary_grh(false);
                context.set_primary_mlid(0); // might be slid
                context.set_primary_sched_queue(
                    DEFAULT_SCHED_QUEUE | ((self.port_number.ok_or("port number not set")? - 1) << 6) | ((attr.ah_attr.sl & 0xf) << 2),
                );
                // TODO: mgid_index, ud_force_mgid, max_stat_rate, hop_limit,
                // TODO: tclass, flow_label, rgid, link_type, if_counter_index

                // set the optional parameters
                // TODO: vsd
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER) {
                        // TODO: check encoding
                        context.set_min_rnr_nak(attr.min_rnr_timer);
                        param_mask.insert(OptionalParameterMask::MIN_RNR_NAK);
                    }
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PKEY_INDEX) {
                    context.set_primary_pkey_index(attr.pkey_index.try_into().unwrap());
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_RC || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE));
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC));
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_READ));
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ALT_PATH) {
                        context.set_alternate_pkey_index(attr.alt_pkey_index.try_into().unwrap());
                        context.set_alternate_rlid(attr.alt_ah_attr.dlid);
                        // TODO: ack_timeout, mgid_index, ud_force_mgid,
                        // TODO: max_stat_rate, hop_limit, tclass, flow_label,
                        // TODO: rgid, link_type, if_counter_index, vlan_index,
                        // TODO: dmac, cv
                        param_mask.insert(OptionalParameterMask::ALTERNATE_PATH);
                    }
                }
                Opcode::Init2RtrQp
            }

            // or just stay in the current state
            (ibv_qp_state::IBV_QPS_INIT, Some(ibv_qp_state::IBV_QPS_INIT)) | (ibv_qp_state::IBV_QPS_INIT, None) => {
                // can update qkey for UD
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                // can update pkey_index
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PKEY_INDEX) {
                    context.set_primary_pkey_index(attr.pkey_index.try_into().unwrap());
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                // can update access flags for RC and UC
                if self.qp_type == ibv_qp_type::IBV_QPT_RC || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE));
                        context.set_remote_atomic(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC));
                        context.set_remote_read(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_READ));
                    }
                }
                Opcode::Init2InitQp
            }

            (ibv_qp_state::IBV_QPS_RTR, Some(ibv_qp_state::IBV_QPS_RTS)) => {
                // set required fields
                // TODO: ack_req_freq, next_send_psn, retry_count
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC));
                    // TODO: check if the devices supports that many outstanding read/atomic operations
                    context.set_sra_max_checked(attr.max_rd_atomic.next_power_of_two().ilog2() as u8).map_err(|_| "sra_max out of bounds")?;
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_RNR_RETRY));
                    context.set_rnr_retry(attr.rnr_retry);
                    assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_TIMEOUT));
                    context.set_primary_ack_timeout(attr.timeout);
                }
                // set optional fields
                // TODO: rate_limit_index
                // TODO: if an alternate path was loaded, we should set
                // path migration state to REARM
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER) {
                        // TODO: check encoding
                        context.set_min_rnr_nak(attr.min_rnr_timer);
                        param_mask.insert(OptionalParameterMask::MIN_RNR_NAK);
                    }
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PKEY_INDEX) {
                    context.set_primary_pkey_index(attr.pkey_index.try_into().unwrap());
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_RC || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    // TODO: remote_read and remote_atomic are invalid optional parameters for UC
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_WRITE));
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC));
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(attr.qp_access_flags.contains(ibv_access_flags::IBV_ACCESS_REMOTE_READ));
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                }
                Opcode::Rtr2RtsQp
            }

            // interestingly, there's no Rtr2RtrQp, but we could emulate it by calling UpdateQp
            (ibv_qp_state::IBV_QPS_RTR, None) => {
                unimplemented!()
            }

            // we can modify values in rts
            (ibv_qp_state::IBV_QPS_RTS, Some(ibv_qp_state::IBV_QPS_RTS)) | (ibv_qp_state::IBV_QPS_RTS, None)  => {
                unimplemented!()
            }

            // ignore SQD for now
            (ibv_qp_state::IBV_QPS_RTS, Some(ibv_qp_state::IBV_QPS_SQD)) => {
                unimplemented!()
            }
            (ibv_qp_state::IBV_QPS_SQD, Some(ibv_qp_state::IBV_QPS_RTS)) => {
                unimplemented!()
            }
            (ibv_qp_state::IBV_QPS_SQD, Some(ibv_qp_state::IBV_QPS_SQD)) | (ibv_qp_state::IBV_QPS_SQD, None) => {
                unimplemented!()
            }

            // resetting is always possible
            (_, Some(ibv_qp_state::IBV_QPS_RESET)) => Opcode::Any2RstQp,

            // There is a command State2State which allows transitioning through multiple States at
            // once, e.g. from INIT to RTS (through RTR) with one command. The Card then does the
            // intermediates transitions automatically. Support has to be checked in the device
            // capabilities, but ConnectX-3 only support 2 variants: INIT to RTS and Reset to RTS

            (ibv_qp_state::IBV_QPS_RESET, Some(_)) => return Err("Can not go from RESET to the supplied State"),
            (ibv_qp_state::IBV_QPS_INIT, Some(_)) => return Err("Can not go from INIT to the supplied State"),
            (ibv_qp_state::IBV_QPS_RTR, Some(_)) => return Err("Can not go from RTR to the supplied State"),
            (ibv_qp_state::IBV_QPS_RTS, Some(_)) => return Err("Can not go from RTS to the supplied State"),
            (ibv_qp_state::IBV_QPS_SQD, Some(_)) => return Err("Can not go from SQD to the supplied State"),
        };
        // actually execute the command
        let mut input = StateTransitionCommandParameter::new_zeroed();
        input.opt_param_mask.set(param_mask.bits());
        input.qpc_data = context.into_bytes();
        cmd.execute_command(opcode, None, InputParam::Mailbox(input.as_bytes()), Some(self.number), OutputParam::Empty)?;
        if let Some(state) = next_qp_state {
            self.state = state;
            trace!("QP {} is now in {:?}", self.number, self.state);
        }
        // TODO: perhaps check if this worked
        Ok(())
    }

    /// Destroy this queue pair.
    pub(super) fn destroy(mut self, cmd: &mut CommandInterface, caps: &Capabilities) -> Result<(), &'static str> {
        trace!("destroying QP {}..", self.number);
        if self.state != ibv_qp_state::IBV_QPS_RESET {
            self.modify(
                cmd,
                caps,
                &ibv_qp_attr {
                    qp_state: ibv_qp_state::IBV_QPS_RESET,
                    ..Default::default()
                },
                ibv_qp_attr_mask::IBV_QP_STATE,
            )?;
        }
        // TODO: deallocate mtt properly
        let _ = self.mtt.take();
        Ok(())
    }

    /// Get the number of this queue pair.
    pub(super) fn number(&self) -> u32 {
        self.number
    }

    /// The UAR page mapped into the calling process, for userspace to ring the SQ doorbell from
    /// directly.
    pub(super) fn uar_page_ptr(&self) -> *mut u8 {
        self.uar_page.start_address().as_mut_ptr()
    }

    /// The BlueFlame page mapped into the calling process, for userspace to post sends through
    /// directly.
    pub(super) fn bf_page_ptr(&self) -> *mut u8 {
        self.bf_page.start_address().as_mut_ptr()
    }
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        if self.mtt.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

#[repr(transparent)]
struct QueuePairDoorbell {
    receive_wqe_index: WriteOnly<u32>,
}

// TODO: why not use a struct instea of a tuple for WorkQueueMeta
type WorkQueueMeta<U, T> = (U, T);

#[derive(Debug)]
struct WorkQueue {
    wqe_cnt: u32,
    max_post: u32,
    max_gs: u32,
    offset: u32,
    wqe_shift: u32,
    spare_wqes: Option<u32>,
    head: u32,
    tail: u32,
    meta: Vec<WorkQueueMeta<u64, u32>>,
    /// Set once this queue's tail has been seen to disagree with the card, see
    /// [`QueuePair::check_wqe_index`]. Only the first disagreement is worth logging.
    divergence_reported: bool,
}

impl WorkQueue {
    /// Compute the size of the receive queue and return it.
    fn new_receive_queue(hca_caps: &Capabilities, ib_caps: &mut ibv_qp_cap) -> Result<Self, &'static str> {
        // check the RQ size before proceeding
        if ib_caps.max_recv_wr > ((1 << u32::from(hca_caps.log_max_qp_sz())) - IB_SQ_MAX_SPARE)
            || ib_caps.max_recv_sge > hca_caps.max_sg_sq().into()
            || ib_caps.max_recv_sge > hca_caps.max_sg_rq().into()
        {
            return Err("RQ size is invalid");
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
        let mut max_post = (1 << u32::from(hca_caps.log_max_qp_sz())) - IB_SQ_MAX_SPARE;
        if max_post > wqe_cnt {
            max_post = wqe_cnt;
        }
        // update the caps
        ib_caps.max_recv_wr = max_post;
        ib_caps.max_recv_sge = *[max_gs, hca_caps.max_sg_sq().into(), hca_caps.max_sg_rq().into()].iter().min().unwrap();
        Ok(Self {
            wqe_cnt,
            max_post,
            max_gs,
            offset: 0,
            wqe_shift,
            spare_wqes: None,
            head: 0,
            tail: 0,
            meta: vec![(0u64, 0u32); wqe_cnt as usize],
            divergence_reported: false,
        })
    }

    /// Compute the size of the receive queue and return it.
    fn new_send_queue(hca_caps: &Capabilities, ib_caps: &mut ibv_qp_cap, qp_type: ibv_qp_type::Type) -> Result<Self, &'static str> {
        // check the SQ size before proceeding
        if ib_caps.max_send_wr > ((1 << u32::from(hca_caps.log_max_qp_sz())) - IB_SQ_MAX_SPARE)
            || ib_caps.max_send_sge > hca_caps.max_sg_sq().into()
            || ib_caps.max_send_sge > hca_caps.max_sg_rq().into()
        {
            return Err("SQ size is invalid");
        }
        let size = ib_caps.max_send_sge * u32::try_from(size_of::<WqeDataSegment>()).unwrap() + send_wqe_overhead(qp_type);
        if size > hca_caps.max_desc_sz_sq().into() {
            return Err("SQ size is invalid");
        }
        let wqe_shift = size.next_power_of_two().ilog2();
        // We need to leave 2 KB + 1 WR of headroom in the SQ to allow HW to prefetch.
        let spare_wqes = ib_sq_headroom(wqe_shift);
        let mut wqe_cnt = ib_caps.max_send_wr;
        if wqe_cnt < 256 {
            wqe_cnt = 256;
        }
        wqe_cnt = (wqe_cnt + spare_wqes).next_power_of_two();
        let max_gs = (u32::from(*[hca_caps.max_desc_sz_sq(), 1 << wqe_shift].iter().min().unwrap()) - send_wqe_overhead(qp_type))
            / u32::try_from(size_of::<WqeDataSegment>()).unwrap();
        let max_post = wqe_cnt - spare_wqes;
        // update the caps
        ib_caps.max_send_wr = max_post;
        ib_caps.max_send_sge = *[max_gs, hca_caps.max_sg_sq().into(), hca_caps.max_sg_rq().into()].iter().min().unwrap();
        Ok(Self {
            wqe_cnt,
            max_post,
            max_gs,
            offset: 0,
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

    /// Get work id based on wqe index
    #[inline(always)]
    fn get_id(&self, wqe_idx: usize) -> u64 {
        let idx = wqe_idx & ((self.wqe_cnt - 1) as usize);
        self.meta[idx].0
    }

    #[inline(always)]
    fn update_id(&mut self, wqe_idx: usize, wr_id: u64) {
        let idx = wqe_idx & ((self.wqe_cnt - 1) as usize);
        self.meta[idx].0 = wr_id;
    }

    /// Get batch size based on wqe index
    #[inline(always)]
    fn get_chain_size(&self, wqe_idx: usize) -> u32 {
        let idx = wqe_idx & ((self.wqe_cnt - 1) as usize);
        self.meta[idx].1
    }

    #[inline(always)]
    fn update_chain_size(&mut self, wqe_idx: usize, batch_size: u32) {
        let idx = wqe_idx & ((self.wqe_cnt - 1) as usize);
        self.meta[idx].1 = batch_size;
    }

    /// Get the `sge_index`th data segment of the WQE at `index`.
    ///
    /// A receive WQE holds `max_gs` data segments, so unlike [`Self::get_element`] this
    /// addresses within a single WQE — adding the segment index to the WQE index would land in
    /// the following WQE instead.
    fn get_data_segment<'e>(
        &self, memory: &'e mut utils::PageToFrameMapping, index: u32, sge_index: u32,
    ) -> Result<&'e mut WqeDataSegment, &'static str> {
        // wrap around
        let index = index & (self.wqe_cnt - 1);
        let (pages, _address) = memory;
        let offset = self.offset + (index << self.wqe_shift) + sge_index * u32::try_from(size_of::<WqeDataSegment>()).unwrap();
        pages.as_type_mut(offset.try_into().unwrap())
    }

    /// Get an element of this work queue.
    ///
    /// The index wraps around to the beginning.
    fn get_element<'e, T: FromBytes>(&self, memory: &'e mut utils::PageToFrameMapping, mut index: u32) -> Result<&'e mut T, &'static str> {
        // wrap around
        index &= self.wqe_cnt - 1;
        let (pages, _addresss) = memory;
        pages.as_type_mut((self.offset + (index << self.wqe_shift)).try_into().unwrap())
    }

    /// Stamp this WQE so that it is invalid if prefetched by marking the
    /// first four bytes of every 64 byte chunk with 0xffffffff, except for
    /// the very first chunk of the WQE.
    ///
    /// This is not part of `WqeControlSegment` because we need to access other
    /// parts of the buffer here.
    fn stamp_wqe(&mut self, memory: &mut utils::PageToFrameMapping, index: u32) -> Result<(), &'static str> {
        let (size, ctrl_address) = {
            let ctrl: &mut WqeControlSegment = self.get_element(memory, index)?;
            let ctrl_address = VirtAddr::new(ctrl as *mut WqeControlSegment as u64);
            (ctrl.size().try_into().unwrap(), ctrl_address)
        };
        let ctrl_offset = memory.0.offset_of_address(ctrl_address).ok_or("control segment has invalid address")?;
        for i in (64..size).step_by(64) {
            let bytes = memory.0.as_slice_mut(ctrl_offset, size)?;
            bytes[i] = u8::MAX;
            bytes[i + 1] = u8::MAX;
            bytes[i + 2] = u8::MAX;
            bytes[i + 3] = u8::MAX;
        }
        Ok(())
    }

    /// Check if this queue would overflow when adding `num_req` work requests.
    fn would_overflow(&self, num_req: u32) -> bool {
        let cur = self.head - self.tail;
        cur + num_req >= self.max_post
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
    fn size(&self) -> u32 {
        (self.vlan_cv_f_ds.get() & 0x3f) << 4
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
struct WqeDataSegment {
    byte_count: U32<BigEndian>,
    lkey: U32<BigEndian>,
    addr: U64<BigEndian>,
}

impl WqeDataSegment {
    /// Copy information from an sge.
    fn copy_from_sge(&mut self, sge: &ibv_sge) -> Result<(), &'static str> {
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
        Ok(())
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
    fn from_wr(wr: &ibv_send_wr_wr) -> Result<Self, &'static str> {
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
            Err("invalid wr field")
        }
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

#[derive(FromBytes)]
#[repr(C)]
struct WqeRemoteAddressSegment {
    va: U64<BigEndian>,
    key: U32<BigEndian>,
    rsvd: u32,
}

impl WqeRemoteAddressSegment {
    /// Create a remote address segment from a wr wr.
    fn from_wr(wr: &ibv_send_wr_wr) -> Result<Self, &'static str> {
        if let ibv_send_wr_wr::rdma { remote_addr, rkey } = wr {
            Ok(Self {
                va: (*remote_addr).into(),
                key: (*rkey).into(),
                rsvd: 0,
            })
        } else {
            Err("invalid wr field")
        }
    }
}

#[bitfield]
struct QueuePairContext {
    state: B4,
    #[skip]
    __: B4,
    service_type: u8,
    #[skip]
    __: B3,
    #[skip(getters)]
    path_migration_state: B2,
    #[skip]
    __: B19,
    #[skip(getters)]
    protection_domain: B24,
    mtu: B3,
    #[skip(getters)]
    msg_max: B5,
    #[skip]
    __: bool,
    log_rq_size: B4,
    log_rq_stride: B3,
    #[skip(getters)]
    sq_no_prefetch: bool,
    log_sq_size: B4,
    log_sq_stride: B3,
    #[skip(getters)]
    roce_mode: B2,
    #[skip]
    __: bool,
    reserved_lkey: bool,
    #[skip]
    __: B12,
    #[skip(getters)]
    usr_page: B24,
    #[skip]
    __: u8,
    local_qpn: B24,
    #[skip]
    __: u8,
    remote_qpn: B24,
    // nested bitfields are only allowed to be 128 bits
    // and nesting bitfields makes them little endian
    #[skip]
    __: B17,
    #[skip(getters)]
    primary_disable_pkey_check: bool,
    #[skip]
    __: B7,
    #[skip(getters)]
    primary_pkey_index: B7,
    #[skip]
    __: u8,
    primary_grh: bool,
    #[skip(getters)]
    primary_mlid: B7,
    primary_rlid: u16,
    primary_ack_timeout: B5,
    #[skip]
    __: B4,
    #[skip(getters)]
    primary_mgid_index: B7,
    #[skip]
    __: u8,
    #[skip(getters)]
    primary_hop_limit: u8,
    #[skip]
    __: B4,
    #[skip(getters)]
    primary_tclass: u8,
    #[skip(getters)]
    primary_flow_label: B20,
    #[skip(getters)]
    primary_rgid: u128,
    #[skip(getters)]
    primary_sched_queue: u8,
    #[skip]
    __: bool,
    #[skip(getters)]
    primary_vlan_index: B7,
    #[skip]
    __: u32,
    #[skip(getters)]
    primary_dmac: B48,
    #[skip]
    __: B17,
    #[skip(getters)]
    alternate_disable_pkey_check: bool,
    #[skip]
    __: B7,
    #[skip(getters)]
    alternate_pkey_index: B7,
    #[skip]
    __: u8,
    #[skip(getters)]
    alternate_grh: bool,
    #[skip(getters)]
    alternate_mlid: B7,
    #[skip(getters)]
    alternate_rlid: u16,
    #[skip(getters)]
    alternate_ack_timeout: B5,
    #[skip]
    __: B4,
    #[skip(getters)]
    alternate_mgid_index: B7,
    #[skip]
    __: u8,
    #[skip(getters)]
    alternate_hop_limit: u8,
    #[skip]
    __: B4,
    #[skip(getters)]
    alternate_tclass: u8,
    #[skip(getters)]
    alternate_flow_label: B20,
    #[skip(getters)]
    alternate_rgid: u128,
    #[skip(getters)]
    alternate_sched_queue: u8,
    #[skip]
    __: bool,
    #[skip(getters)]
    alternate_vlan_index: B7,
    #[skip]
    __: u32,
    #[skip(getters)]
    alternate_dmac: B48,
    #[skip]
    __: u8,
    #[skip(getters)]
    sra_max: B3,
    #[skip]
    __: B5,
    rnr_retry: B3,
    #[skip]
    __: B53,
    #[skip(getters)]
    next_send_psn: B24,
    #[skip]
    __: u8,
    #[skip(getters)]
    cqn_send: B24,
    #[skip(getters)]
    roce_entropy: u16,
    #[skip]
    __: B56,
    #[skip(getters)]
    last_acked_psn: B24,
    #[skip]
    __: u8,
    #[skip(getters)]
    ssn: B24,
    #[skip]
    __: u8,
    #[skip(getters)]
    rra_max: B3,
    #[skip]
    __: B5,
    #[skip(getters)]
    remote_read: bool,
    #[skip(getters)]
    remote_write: bool,
    #[skip(getters)]
    remote_atomic: bool,
    #[skip]
    __: B16,
    min_rnr_nak: B5,
    #[skip(getters)]
    next_recv_psn: B24,
    #[skip]
    __: u16,
    #[skip(getters)]
    xrcd: u16,
    #[skip]
    __: u8,
    #[skip(getters)]
    cqn_receive: B24,
    /// The last three bits must be zero.
    db_record_addr: u64,
    qkey: u32,
    #[skip]
    __: u8,
    srqn: B24,
    #[skip]
    __: u8,
    #[skip(getters)]
    msn: B24,
    rq_wqe_counter: u16,
    sq_wqe_counter: u16,
    // rate_limit_params
    #[skip]
    __: B56,
    #[skip(getters)]
    qos_vport: u8,
    #[skip]
    __: u32,
    #[skip(getters)]
    num_rmc_peers: u8,
    #[skip(getters)]
    base_mkey: B24,
    #[skip]
    __: B2,
    #[skip(getters)]
    log_page_size: B6,
    #[skip]
    __: u16,
    /// The last three bits must be zero.
    mtt_base_addr: B40,
    #[skip]
    __: u128,
    #[skip]
    __: u128,
    #[skip]
    __: u64,
}

impl core::fmt::Debug for QueuePairContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QueuePairContext")
            .field("state", &self.state())
            .field("MTU", &ibv_mtu::from_repr(self.mtu()))
            .field("QKEY", &self.qkey())
            .field("QP Number", &self.local_qpn())
            .field("Send Counter", &self.sq_wqe_counter())
            .field("Receive Counter", &self.rq_wqe_counter())
            .field("service type", &self.service_type())
            .field("remote QPN", &self.remote_qpn())
            .field("primary rlid", &self.primary_rlid())
            .field("primary grh", &self.primary_grh())
            .field("primary ack timeout", &self.primary_ack_timeout())
            .field("rnr retry", &self.rnr_retry())
            .field("min rnr nak", &self.min_rnr_nak())
            .field("log sq size/stride", &(self.log_sq_size(), self.log_sq_stride()))
            .field("log rq size/stride", &(self.log_rq_size(), self.log_rq_stride()))
            .field("srqn", &self.srqn())
            .field("reserved lkey", &self.reserved_lkey())
            .field("mtt base addr", &self.mtt_base_addr())
            .field("db record addr", &self.db_record_addr())
            .finish_non_exhaustive()
    }
}

#[derive(AsBytes, FromBytes)]
#[repr(C, packed)]
struct StateTransitionCommandParameter {
    opt_param_mask: U32<BigEndian>,
    _reserved: u32,
    qpc_data: [u8; 248],
    _reserved2: [u8; 252],
}

bitflags! {
    struct OptionalParameterMask: u32 {
        const ALTERNATE_PATH = 1 << 0;
        const REMOTE_READ = 1 << 1;
        const REMOTE_ATOMIC = 1 << 2;
        const REMOTE_WRITE = 1 << 3;
        const PKEY_INDEX = 1 << 4;
        const QKEY = 1 << 5;
        const MIN_RNR_NAK = 1 << 6;
    }
}

#[repr(u32)]
#[derive(FromRepr)]
pub(super) enum QueuePairOpcode {
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
