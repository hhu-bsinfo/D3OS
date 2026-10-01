//! This module consists of functions that create, work with and destroy queue
//! pairs. Its functions can change the state of a QP and query and print some
//! QP infos.

use core::mem::size_of;

use super::{
    Mlx4Device, PdHandle,
    cmd::{CommandInterface, Opcode},
    device::{PAGE_SHIFT, uar_index_to_hw},
    fw::Capabilities,
    icm::ICM_PAGE_SHIFT,
};
use crate::device::infiniband::mlx4::cmd::{InputParam, OutputParam};
use crate::process::process::Process;
use alloc::sync::Arc;
use bitflags::bitflags;
use byteorder::BigEndian;
use log::trace;
use modular_bitfield_msb::{bitfield, prelude::*};
use rdma::{AccessFlags, Mtu, QueuePairAttr, QueuePairAttrMask, QueuePairType, QueuePairState};
use uuid::Uuid;
use x86_64::structures::paging::{Page, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};
use zerocopy::{AsBytes, FromBytes, U32};

#[derive(Debug)]
pub(super) struct QueuePair {
    number: u32,
    owner: Uuid,
    state: QueuePairState,
    qp_type: QueuePairType,
    port_number: Option<u8>,
    pd: PdHandle,
    // TODO: bind the lifetime to the one of the completion queues
    send_cq_number: u32,
    receive_cq_number: u32,
    uar_index: u32,
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
        dev: &mut Mlx4Device, process: Arc<Process>, qp_type: QueuePairType, pd: PdHandle, send_cq_number: u32, receive_cq_number: u32, buffer: *const u8,
        doorbell_ptr: *const u32, uar_index: u32, log_sq_bb_count: u8, log_sq_stride: u8, log_rq_wqe_count: u8, log_rq_stride: u8,
    ) -> Result<Self, &'static str> {
        let doorbell_addr = VirtAddr::try_new(doorbell_ptr as u64).map_err(|_| "Doorbell address is not canonical")?;
        if !process.virtual_address_space.access_ok(doorbell_addr, size_of::<u32>()) {
            return Err("User has no access to Doorbell");
        }

        // The context stores each stride as `log_stride - 4` in 3 bits.
        if log_sq_stride < 4 || log_rq_stride < 4 {
            return Err("stride is not multiple of 16 bytes");
        }
        if log_sq_stride > 11 || log_rq_stride > 11 {
            return Err("stride is larger than 2048 bytes");
        }

        // Bound WQE counts against the HCA's max QP size and the context's 4-bit size fields.
        let log_max_qp_sz = dev.capabilities.log_max_qp_sz().min(15);
        if log_sq_bb_count > log_max_qp_sz || log_rq_wqe_count > log_max_qp_sz {
            return Err("WQE count exceeds the HCA's max QP size");
        }

        let buffer_size: u64 = (1 << (log_sq_bb_count + log_sq_stride)) + (1 << (log_rq_wqe_count + log_rq_stride));
        let buffer_addr = VirtAddr::try_new(buffer as u64).map_err(|_| "Buffer address is not canonical")?;

        if !buffer_addr.is_aligned(64u64) {
            return Err("buffer is not aligned to 64");
        }
        // Checked before computing the page range, which would panic past the canonical range.
        if !process.virtual_address_space.access_ok(buffer_addr, buffer_size as usize) {
            return Err("User has no access to QP buffer");
        }
        let start: Page<Size4KiB> = Page::containing_address(buffer_addr);
        let end = start + buffer_size.div_ceil(start.size());
        let mtt = Some(
            dev.icm_tables
                .memory_regions()
                .alloc_mtt_for_pages(&dev.capabilities, Page::range(start, end))?,
        );
        // Offset from the first page in units of 64 bytes
        let page_offset: u8 = ((buffer_addr - start.start_address()) >> 6) as u8;

        let doorbell_address = process
            .virtual_address_space
            .get_phys(doorbell_ptr as u64)
            .ok_or("doorbell not mapped to physical address")?;

        // TODO: make sure ICM entry for this queue pair number is mapped to physical memory
        let number = dev.offsets.alloc_qpn().try_into().unwrap();

        let qp = Self {
            number,
            owner: process.id(),
            state: QueuePairState::Reset,
            qp_type,
            port_number: None,
            pd,
            send_cq_number,
            receive_cq_number,
            uar_index,
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
    // Not wired up yet; this is what ibv_query_qp would use.
    #[allow(dead_code)]
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
        &mut self, cmd: &mut CommandInterface, caps: &Capabilities, attr: &QueuePairAttr, attr_mask: QueuePairAttrMask,
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

        // Every attribute comes straight from userspace, so a missing or out-of-range one is an
        // error for the caller, never a panic.
        if attr_mask.contains(QueuePairAttrMask::IBV_QP_PORT) && !(1..=caps.num_ports()).contains(&attr.port_num) {
            return Err("port number out of range");
        }

        let next_qp_state = if attr_mask.contains(QueuePairAttrMask::IBV_QP_STATE) {
            Some(attr.qp_state)
        } else {
            None
        };

        // get the right state transition
        let opcode = match (self.state, next_qp_state) {
            // initialize
            (QueuePairState::Reset, Some(QueuePairState::Init)) => {
                // The port belongs to INIT2RTR, but applications may already set it here.
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PORT) {
                    self.port_number = Some(attr.port_num);
                }
                // set required fields
                context.set_service_type(match self.qp_type {
                    QueuePairType::RC => 0x0,
                    QueuePairType::UC => 0x1,
                    QueuePairType::UD => 0x3,
                    #[allow(unreachable_patterns)]
                    _ => return Err("invalid queue pair type"),
                });
                context.set_path_migration_state(PATH_MIGRATION_STATE_MIGRATED);
                context.set_usr_page(uar_index_to_hw(self.uar_index).try_into().unwrap());
                context.set_protection_domain(self.pd.0);
                context.set_cqn_send(self.send_cq_number);
                // RC needs remote read
                if self.qp_type == QueuePairType::RC {
                    // TODO: this might have been set in an earlier call
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        return Err("access flags are required");
                    }
                    context.set_remote_read(attr.qp_access_flags.contains(AccessFlags::REMOTE_READ));
                }
                // RC and UC need remote write
                if self.qp_type == QueuePairType::RC || self.qp_type == QueuePairType::UC {
                    // TODO: this might have been set in an earlier call
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        return Err("access flags are required");
                    }
                    context.set_remote_write(attr.qp_access_flags.contains(AccessFlags::REMOTE_WRITE));
                }
                // RC needs remote atomic
                if self.qp_type == QueuePairType::RC {
                    // TODO: this might have been set in an earlier call
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        return Err("access flags are required");
                    }
                    context.set_remote_atomic(attr.qp_access_flags.contains(AccessFlags::REMOTE_ATOMIC));
                }
                context.set_cqn_receive(self.receive_cq_number);
                // UD needs qkey
                if self.qp_type == QueuePairType::UD {
                    // TODO: this might have been set in an earlier call
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_QKEY) {
                        return Err("qkey is required");
                    }
                    context.set_qkey(attr.qkey);
                }
                // TODO: RC and UD need srq
                // TODO: RC and UD need srqn
                // TODO: fre
                context.set_log_sq_size(self.log_sq_bb_count);
                context.set_log_rq_size(self.log_rq_wqe_count);
                context.set_log_sq_stride(self.log_sq_stride - 4);
                context.set_log_rq_stride(self.log_rq_stride - 4);
                context.set_reserved_lkey(false);
                // TODO: sq_wqe_counter, rq_wqe_counter, is
                // TODO: hs, vsd, rss for UD
                context.set_sq_no_prefetch(false);
                context.set_page_offset(self.page_offset);
                // TODO: pkey_index, disable_pkey_check
                // TOODO: rss context for UD
                context.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
                context.set_mtt_base_addr(self.mtt.ok_or("queue pair has no MTT")?);
                context.set_db_record_addr(self.doorbell_address.as_u64().try_into().unwrap());

                // Userspace initializes the SQ ownership bits and headroom before requesting this.

                Opcode::Rst2InitQp
            }

            // or just stay in the current state
            // We can't even set anything here.
            (QueuePairState::Reset, None) => return Ok(()),

            // init -> rtr
            (QueuePairState::Init, Some(QueuePairState::ReadyToReceive)) => {
                // we need the port number for this transition
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PORT) {
                    self.port_number = Some(attr.port_num);
                }

                // set required fields
                // TODO: this might have been set in an earlier call
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PATH_MTU) {
                    context.set_mtu(attr.path_mtu as u8);
                } else {
                    // default to the highest one
                    context.set_mtu(Mtu::default() as u8);
                }
                context.set_msg_max(caps.log_max_msg());

                // TODO: required parameters for RC and UC: next_recv_psn, qos_vport, roce_mode,
                if self.qp_type == QueuePairType::RC || self.qp_type == QueuePairType::UC {
                    // TODO: this might have been set in an earlier call
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_DEST_QPN) {
                        return Err("destination QPN is required");
                    }
                    context.set_remote_qpn_checked(attr.dest_qp_num).map_err(|_| "destination QPN out of range")?;
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_AV) {
                        return Err("address vector is required");
                    }
                    context.set_primary_rlid(attr.ah_attr.dlid);
                }

                // TODO: required parameters for RC: ric
                if self.qp_type == QueuePairType::RC {
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_MAX_DEST_RD_ATOMIC) {
                        return Err("max_dest_rd_atomic is required");
                    }
                    // TODO: check if the devices supports that many outstanding read/atomic operations
                    let rra_max = attr.max_dest_rd_atomic.checked_next_power_of_two().ok_or("rra_max out of bounds")?;
                    context.set_rra_max_checked(rra_max.ilog2() as u8).map_err(|_| "rra_max out of bounds")?;
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
                if self.qp_type == QueuePairType::RC {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_MIN_RNR_TIMER) {
                        // TODO: check encoding
                        context.set_min_rnr_nak_checked(attr.min_rnr_timer).map_err(|_| "min_rnr_timer out of range")?;
                        param_mask.insert(OptionalParameterMask::MIN_RNR_NAK);
                    }
                }
                if self.qp_type == QueuePairType::UD {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PKEY_INDEX) {
                    context
                        .set_primary_pkey_index_checked(attr.pkey_index.try_into().map_err(|_| "pkey index out of range")?)
                        .map_err(|_| "pkey index out of range")?;
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == QueuePairType::RC || self.qp_type == QueuePairType::UC {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(AccessFlags::REMOTE_WRITE));
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(attr.qp_access_flags.contains(AccessFlags::REMOTE_ATOMIC));
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(attr.qp_access_flags.contains(AccessFlags::REMOTE_READ));
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_ALT_PATH) {
                        context
                            .set_alternate_pkey_index_checked(attr.alt_pkey_index.try_into().map_err(|_| "alternate pkey index out of range")?)
                            .map_err(|_| "alternate pkey index out of range")?;
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
            (QueuePairState::Init, Some(QueuePairState::Init)) | (QueuePairState::Init, None) => {
                // can update qkey for UD
                if self.qp_type == QueuePairType::UD {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                // can update pkey_index
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PKEY_INDEX) {
                    context
                        .set_primary_pkey_index_checked(attr.pkey_index.try_into().map_err(|_| "pkey index out of range")?)
                        .map_err(|_| "pkey index out of range")?;
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                // can update access flags for RC and UC
                if self.qp_type == QueuePairType::RC || self.qp_type == QueuePairType::UC {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(AccessFlags::REMOTE_WRITE));
                        context.set_remote_atomic(attr.qp_access_flags.contains(AccessFlags::REMOTE_ATOMIC));
                        context.set_remote_read(attr.qp_access_flags.contains(AccessFlags::REMOTE_READ));
                    }
                }
                Opcode::Init2InitQp
            }

            (QueuePairState::ReadyToReceive, Some(QueuePairState::ReadyToSend)) => {
                // set required fields
                // TODO: ack_req_freq, next_send_psn, retry_count
                if self.qp_type == QueuePairType::RC {
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_MAX_QP_RD_ATOMIC) {
                        return Err("max_rd_atomic is required");
                    }
                    // TODO: check if the devices supports that many outstanding read/atomic operations
                    let sra_max = attr.max_rd_atomic.checked_next_power_of_two().ok_or("sra_max out of bounds")?;
                    context.set_sra_max_checked(sra_max.ilog2() as u8).map_err(|_| "sra_max out of bounds")?;
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_RNR_RETRY) {
                        return Err("rnr_retry is required");
                    }
                    context.set_rnr_retry_checked(attr.rnr_retry).map_err(|_| "rnr_retry out of range")?;
                    if !attr_mask.contains(QueuePairAttrMask::IBV_QP_TIMEOUT) {
                        return Err("timeout is required");
                    }
                    context.set_primary_ack_timeout_checked(attr.timeout).map_err(|_| "timeout out of range")?;
                }
                // set optional fields
                // TODO: rate_limit_index
                // TODO: if an alternate path was loaded, we should set
                // path migration state to REARM
                if self.qp_type == QueuePairType::RC {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_MIN_RNR_TIMER) {
                        // TODO: check encoding
                        context.set_min_rnr_nak_checked(attr.min_rnr_timer).map_err(|_| "min_rnr_timer out of range")?;
                        param_mask.insert(OptionalParameterMask::MIN_RNR_NAK);
                    }
                }
                if self.qp_type == QueuePairType::UD {
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                if attr_mask.contains(QueuePairAttrMask::IBV_QP_PKEY_INDEX) {
                    context
                        .set_primary_pkey_index_checked(attr.pkey_index.try_into().map_err(|_| "pkey index out of range")?)
                        .map_err(|_| "pkey index out of range")?;
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == QueuePairType::RC || self.qp_type == QueuePairType::UC {
                    // TODO: remote_read and remote_atomic are invalid optional parameters for UC
                    if attr_mask.contains(QueuePairAttrMask::IBV_QP_ACCESS_FLAGS) {
                        context.set_remote_write(attr.qp_access_flags.contains(AccessFlags::REMOTE_WRITE));
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(attr.qp_access_flags.contains(AccessFlags::REMOTE_ATOMIC));
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(attr.qp_access_flags.contains(AccessFlags::REMOTE_READ));
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                }
                Opcode::Rtr2RtsQp
            }

            // interestingly, there's no Rtr2RtrQp, but we could emulate it by calling UpdateQp
            (QueuePairState::ReadyToReceive, None) => return Err("modifying a QP in RTR is not supported"),

            // we could modify values in rts
            (QueuePairState::ReadyToSend, Some(QueuePairState::ReadyToSend)) | (QueuePairState::ReadyToSend, None) => {
                return Err("modifying a QP in RTS is not supported");
            }

            // ignore SQD for now
            (QueuePairState::ReadyToSend, Some(QueuePairState::SQD))
            | (QueuePairState::SQD, Some(QueuePairState::ReadyToSend))
            | (QueuePairState::SQD, Some(QueuePairState::SQD))
            | (QueuePairState::SQD, None) => return Err("the SQD state is not supported"),

            // resetting is always possible
            (_, Some(QueuePairState::Reset)) => Opcode::Any2RstQp,

            // There is a command State2State which allows transitioning through multiple States at
            // once, e.g. from INIT to RTS (through RTR) with one command. The Card then does the
            // intermediates transitions automatically. Support has to be checked in the device
            // capabilities, but ConnectX-3 only support 2 variants: INIT to RTS and Reset to RTS
            (QueuePairState::Reset, Some(_)) => return Err("Can not go from RESET to the supplied State"),
            (QueuePairState::Init, Some(_)) => return Err("Can not go from INIT to the supplied State"),
            (QueuePairState::ReadyToReceive, Some(_)) => return Err("Can not go from RTR to the supplied State"),
            (QueuePairState::ReadyToSend, Some(_)) => return Err("Can not go from RTS to the supplied State"),
            (QueuePairState::SQD, Some(_)) => return Err("Can not go from SQD to the supplied State"),
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
        if self.state != QueuePairState::Reset {
            self.modify(
                cmd,
                caps,
                &QueuePairAttr {
                    qp_state: QueuePairState::Reset,
                    ..Default::default()
                },
                QueuePairAttrMask::IBV_QP_STATE,
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

    /// The process that created this queue pair.
    pub(super) fn owner(&self) -> Uuid {
        self.owner
    }
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        if self.mtt.is_some() {
            panic!("please destroy instead of dropping")
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

    // 0x00090
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
    __: bool,
    #[skip(getters)]
    page_offset: B6,
    #[skip]
    __: B6,

    // 0x00094
    #[skip]
    __: B3,
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
            .field("MTU", &Mtu::from_repr(self.mtu()))
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

