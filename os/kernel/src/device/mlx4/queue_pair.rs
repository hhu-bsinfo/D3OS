//! This module consists of functions that create, work with and destroy queue
//! pairs. Its functions can change the state of a QP and query and print some
//! QP infos.

use core::{mem::size_of, sync::atomic::{compiler_fence, Ordering}};

use bitflags::bitflags;
use byteorder::BigEndian;
use crate::infiniband::ib_core::{ibv_access_flags, ibv_mtu, ibv_qp_attr, ibv_qp_attr_mask, ibv_qp_cap, ibv_qp_state, ibv_qp_type, ibv_recv_wr, ibv_send_wr, ibv_send_wr_wr, ibv_sge, ibv_wr_opcode};
use modular_bitfield_msb::{bitfield, prelude::{B12, B16, B17, B19, B2, B20, B24, B3, B4, B40, B48, B5, B53, B56, B6, B7}};
use strum_macros::FromRepr;
use volatile::WriteOnly;
use x86_64::{PhysAddr, VirtAddr};
use zerocopy::{AsBytes, FromBytes, U16, U32, U64};
use crate::memory::PAGE_SIZE;
use log::trace;

use super::{
    cmd::{CommandInterface, Opcode},
    completion_queue::CompletionQueue,
    device::{uar_index_to_hw, PAGE_SHIFT},
    fw::{Capabilities, DoorbellPage},
    icm::{ICM_PAGE_SHIFT, MrTable},
    mlx4_ib::Offsets,
    utils,
    utils::{MappedPages, Operations}
};

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
    // TODO: this seems deprecated
    _is_special: bool,
    sq: WorkQueue,
    rq: WorkQueue,
    // TODO: bind the lifetime to the one of the completion queues
    send_cq_number: u32,
    receive_cq_number: u32,
    memory: Option<utils::PageToFrameMapping>,
    uar_idx: usize,
    doorbell_page: MappedPages,
    doorbell_address: PhysAddr,
    mtt: u64,
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
        cmd: &mut CommandInterface, caps: &Capabilities, offsets: &mut Offsets,
        memory_regions: &mut MrTable, qp_type: ibv_qp_type::Type,
        send_cq: &CompletionQueue, receive_cq: &CompletionQueue,
        ib_caps: &mut ibv_qp_cap,
    ) -> Result<Self, &'static str> {
        let number = offsets.alloc_qpn().try_into().unwrap();
        let uar_idx = offsets.alloc_scq_db();
        let state = ibv_qp_state::IBV_QPS_RESET;
        let send_cq_number = send_cq.number();
        let receive_cq_number = receive_cq.number();
        let mut rq = WorkQueue::new_receive_queue(caps, ib_caps)?;
        let mut sq = WorkQueue::new_send_queue(caps, ib_caps, qp_type)?;
        if rq.wqe_shift > sq.wqe_shift {
            rq.offset = 0;
            sq.offset = rq.size();
        } else {
            rq.offset = sq.size();
            sq.offset = 0;
        }
        let buf_size = (rq.size() + sq.size()).try_into().unwrap();
        let mut operation_container = Operations::default();
        let mapped_page_to_frame = utils::create_cont_mapping_with_dma_flags(
            utils::pages_required(buf_size))?.fetch_in_addr()?;
        let bytes = utils::start_page_as_mut_ptr::<u8>(mapped_page_to_frame.0.into_range().start);
        // zero the queue
        operation_container.create_fill(&(0u8, bytes, buf_size));
        operation_container.perform();
        
        let mtt = memory_regions.alloc_mtt(
            cmd, caps, buf_size / PAGE_SIZE, mapped_page_to_frame.1,
        )?;
        let (mut doorbell_page, doorbell_address) = utils::create_cont_mapping_with_dma_flags(
            utils::pages_required(size_of::<QueuePairDoorbell>()))?.fetch_in_addr()?;
        let doorbell: &mut QueuePairDoorbell = doorbell_page
            .as_type_mut(0)?;
        doorbell.receive_wqe_index.write(0.into());
        let qp = Self {
            number, state, qp_type, port_number: None, _is_special: false, sq,
            rq, send_cq_number, receive_cq_number, memory: Some(mapped_page_to_frame), uar_idx,
            doorbell_page, doorbell_address, mtt,
        };
        trace!("created new QP: {qp:?}");
        Ok(qp)
    }

    /// Query this queue pair.
    pub(super) fn query(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        let page: MappedPages = cmd.execute_command(
            Opcode::QueryQp, (), (), self.number,
        )?;
        let transition: &StateTransitionCommandParameter = page.as_type(0)?;
        let context = QueuePairContext::from_bytes(transition.qpc_data);
        trace!("Queue Pair Context: {context:?}");
        Ok(())
    }

    /// Modify this queue pair.
    /// 
    /// This is used by ibv_modify_qp.
    pub(super) fn modify(
        &mut self, cmd: &mut CommandInterface, caps: &Capabilities,
        attr: &ibv_qp_attr, attr_mask: ibv_qp_attr_mask,
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
        // get the right state transition
        let opcode = match (self.state, attr_mask.contains(
                ibv_qp_attr_mask::IBV_QP_STATE
        ), attr.qp_state) {
            // initialize
            (ibv_qp_state::IBV_QPS_RESET, true, ibv_qp_state::IBV_QPS_INIT) => {
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
                    _ => return Err("invalid queue pair type"),
                });
                context.set_path_migration_state(PATH_MIGRATION_STATE_MIGRATED);
                context.set_usr_page(uar_index_to_hw(
                    self.uar_idx
                ).try_into().unwrap());
                // TODO: protection domain
                context.set_cqn_send(self.send_cq_number);
                // RC needs remote read
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ));
                    context.set_remote_read(attr.qp_access_flags.contains(
                        ibv_access_flags::IBV_ACCESS_REMOTE_READ
                    ));
                }
                // RC and UC need remote write
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                    || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ));
                    context.set_remote_write(attr.qp_access_flags.contains(
                        ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                    ));
                }
                // RC needs remote atomic
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ));
                    context.set_remote_atomic(
                        attr.qp_access_flags.contains(
                            ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC
                        )
                    );
                }
                context.set_cqn_receive(self.receive_cq_number);
                // UD needs qkey
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_QKEY
                    ));
                    context.set_qkey(attr.qkey);
                }
                // TODO: RC and UD need srq
                // TODO: RC and UD need srqn
                // TODO: fre
                assert_ne!(self.sq.wqe_cnt, 0);
                context.set_log_sq_size(
                    self.sq.wqe_cnt.ilog2().try_into().unwrap()
                );
                assert_ne!(self.rq.wqe_cnt, 0);
                context.set_log_rq_size(
                    self.rq.wqe_cnt.ilog2().try_into().unwrap()
                );
                context.set_log_sq_stride(
                    (self.sq.wqe_shift - 4).try_into().unwrap()
                );
                context.set_log_rq_stride(
                    (self.rq.wqe_shift - 4).try_into().unwrap()
                );
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
                context.set_mtt_base_addr(self.mtt);
                context.set_db_record_addr(
                    self.doorbell_address.as_u64().try_into().unwrap()
                );

                // Before passing a kernel QP to the HW, make sure that the
                // ownership bits of the send queue are set and the SQ headroom
                // is stamped so that the hardware doesn't start processing
                // stale work requests.
                let memory = self.memory.as_mut().unwrap();
                for i in 0..self.sq.wqe_cnt {
                    let ctrl: &mut WqeControlSegment = self.sq.get_element(
                        memory, i,
                    )?;
                    ctrl.owner_opcode = (1 << 31).into();
                    ctrl.vlan_cv_f_ds = u32::to_be(
                        1 << (self.sq.wqe_shift - 4)
                    ).into();
                    self.sq.stamp_wqe(memory, i)?;
                }
                Opcode::Rst2InitQp
            },
            // or just stay in the current state
            // We can't even set anything here.
            (ibv_qp_state::IBV_QPS_RESET, false, _) => Opcode::Any2RstQp,

            // init -> rtr
            (ibv_qp_state::IBV_QPS_INIT, true, ibv_qp_state::IBV_QPS_RTR) => {
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
                    context.set_mtu(ibv_mtu::Mtu4096 as u8);
                }
                context.set_msg_max(caps.log_max_msg());
                // RC and UC need remote_qpn
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                    || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    // TODO: this might have been set in an earlier call
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_DEST_QPN
                    ));
                    context.set_remote_qpn(attr.dest_qp_num);
                }
                // TODO: rra_max, ric, next_recv_psn, qos_vport, roce_mode,
                // TODO: rate_limit_index
                assert!(attr_mask.contains(ibv_qp_attr_mask::IBV_QP_AV));
                // RC and RC need rlid
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                    || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    context.set_primary_rlid(attr.ah_attr.dlid);
                }
                context.set_primary_grh(false);
                context.set_primary_mlid(0); // might be slid
                context.set_primary_sched_queue(
                    DEFAULT_SCHED_QUEUE
                        | ((self.port_number.ok_or("port number not set")? - 1) << 6)
                        | ((attr.ah_attr.sl & 0xf) << 2)
                );
                // TODO: mgid_index, ud_force_mgid, max_stat_rate, hop_limit,
                // TODO: tclass, flow_label, rgid, link_type, if_counter_index
                // set the optional parameters
                // TODO: vsd
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    if attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER
                    ) {
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
                    context.set_primary_pkey_index(
                        attr.pkey_index.try_into().unwrap()
                    );
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                    || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    if attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ) {
                        context.set_remote_write(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_READ
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_ALT_PATH) {
                        context.set_alternate_pkey_index(
                            attr.alt_pkey_index.try_into().unwrap()
                        );
                        context.set_alternate_rlid(attr.alt_ah_attr.dlid);
                        // TODO: ack_timeout, mgid_index, ud_force_mgid,
                        // TODO: max_stat_rate, hop_limit, tclass, flow_label,
                        // TODO: rgid, link_type, if_counter_index, vlan_index,
                        // TODO: dmac, cv
                        param_mask.insert(
                            OptionalParameterMask::ALTERNATE_PATH
                        );
                    }
                }
                Opcode::Init2RtrQp
            }
            // or just stay in the current state
            (ibv_qp_state::IBV_QPS_INIT, true, ibv_qp_state::IBV_QPS_INIT)
             | (ibv_qp_state::IBV_QPS_INIT,false, _) => {
                // can update qkey for UD
                if self.qp_type == ibv_qp_type::IBV_QPT_UD {
                    if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_QKEY) {
                        context.set_qkey(attr.qkey);
                        param_mask.insert(OptionalParameterMask::QKEY);
                    }
                }
                // can update pkey_index
                if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_PKEY_INDEX) {
                    context.set_primary_pkey_index(
                        attr.pkey_index.try_into().unwrap()
                    );
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                // can update access flags for RC and UC
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                    || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    if attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ) {
                        context.set_remote_write(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                            )
                        );
                        context.set_remote_atomic(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC
                            )
                        );
                        context.set_remote_read(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_READ
                            )
                        );
                    }
                }
                Opcode::Init2InitQp
            },

            // rtr -> rts
            (ibv_qp_state::IBV_QPS_RTR, true, ibv_qp_state::IBV_QPS_RTS) => {
                // set required fields
                // TODO: ack_req_freq, sra_max, next_send_psn, retry_count
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_RNR_RETRY
                    ));
                    context.set_rnr_retry(attr.rnr_retry);
                    assert!(attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_TIMEOUT
                    ));
                    context.set_primary_ack_timeout(attr.timeout);
                }
                // set optional fields
                // TODO: rate_limit_index
                // TODO: if an alternate path was loaded, we should set
                // path migration state to REARM
                if self.qp_type == ibv_qp_type::IBV_QPT_RC {
                    if attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER
                    ) {
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
                    context.set_primary_pkey_index(
                        attr.pkey_index.try_into().unwrap()
                    );
                    param_mask.insert(OptionalParameterMask::PKEY_INDEX);
                }
                if self.qp_type == ibv_qp_type::IBV_QPT_RC
                 || self.qp_type == ibv_qp_type::IBV_QPT_UC {
                    if attr_mask.contains(
                        ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
                    ) {
                        context.set_remote_write(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_WRITE);
                        context.set_remote_atomic(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_ATOMIC);
                        context.set_remote_read(
                            attr.qp_access_flags.contains(
                                ibv_access_flags::IBV_ACCESS_REMOTE_READ
                            )
                        );
                        param_mask.insert(OptionalParameterMask::REMOTE_READ);
                    }
                }
                Opcode::Rtr2RtsQp
            },
            // interestingly, there's no Rtr2RtrQp
            // but we could emulate it by calling UpdateQp

            // we can modify values in rts
            (ibv_qp_state::IBV_QPS_RTS, true, ibv_qp_state::IBV_QPS_RTS)
             | (ibv_qp_state::IBV_QPS_RTS, false, _) => {
                todo!()
            }

            // ignore SQD for now
            (ibv_qp_state::IBV_QPS_RTS, true, ibv_qp_state::IBV_QPS_SQD) => {
                unimplemented!()
            },
            (ibv_qp_state::IBV_QPS_SQD, true, ibv_qp_state::IBV_QPS_RTS) => {
                unimplemented!()
            },
            (ibv_qp_state::IBV_QPS_SQD, true, ibv_qp_state::IBV_QPS_SQD)
             | (ibv_qp_state::IBV_QPS_SQD, false, _) => {
                unimplemented!()
            },

            // resetting is always possible
            (_, true, ibv_qp_state::IBV_QPS_RESET) => Opcode::Any2RstQp,
            // nothing else is possible
            _ => return Err("invalid state transition"),
        };
        // actually execute the command
        let mut input = StateTransitionCommandParameter::new_zeroed();
        input.opt_param_mask.set(param_mask.bits());
        input.qpc_data = context.into_bytes();
        let _ : () = cmd.execute_command(
            opcode, (), input.as_bytes(), self.number,
        )?;
        if attr_mask.contains(ibv_qp_attr_mask::IBV_QP_STATE) {
            self.state = attr.qp_state;
            trace!("QP {} is now in {:?}", self.number, self.state);
        }
        // TODO: perhaps check if this worked
        Ok(())
    }

    /// Post a work request to receive data.
    /// 
    /// This is used by ibv_post_recv.
    pub(super) fn post_receive(
        &mut self, wr: &mut ibv_recv_wr
    ) -> Result<(), &'static str> {
        if self.state != ibv_qp_state::IBV_QPS_RTR
         && self.state != ibv_qp_state::IBV_QPS_RTS {
            return Err("queue pair cannot receive in this state");
        }
        let mut index = self.rq.head;
        let mut current = Some(wr);
        let mut num_req = 0;
        while current.is_some() {
            let curr = current.take().unwrap();
            // make sure that we're not overflowing
            if self.rq.would_overflow(num_req) {
                return Err("receive queue would overflow");
            }
            // check that this work request is not too big
            if u32::try_from(curr.num_sge).unwrap() > self.rq.max_gs {
                return Err("work request has too many sges");
            }
            let mut sge_index = 0;
            for sge in &curr.sg_list {
                let elem: &mut WqeDataSegment = self.rq.get_element(
                    self.memory.as_mut().unwrap(), index + sge_index,
                )?;
                elem.copy_from_sge(sge)?;
                sge_index += 1;
            }
            // fill the last one
            let last_elem: &mut WqeDataSegment = self.rq.get_element(
                    self.memory.as_mut().unwrap(), index + sge_index,
            )?;
            *last_elem = WqeDataSegment::last();
            num_req += 1;
            index += 1;
            // TODO: support multiple work requests
            assert!(curr.next.is_none());
        }
        // return if we don't have anything to do
        if num_req == 0 {
            return Ok(());
        }
        self.rq.head += num_req;
        // make sure that the descriptors are written before the doorbell
        compiler_fence(Ordering::SeqCst);
        let doorbell: &mut QueuePairDoorbell = self.doorbell_page
            .as_type_mut(0)?;
        doorbell.receive_wqe_index.write(
            (self.rq.head as u16).into() // wrap around at u16::MAX
        );
        Ok(())
    }

    /// Post a work request to send data.
    /// 
    /// This is used by ibv_post_send.
    pub(super) fn post_send(
        &mut self, caps: &Capabilities, doorbells: &mut [MappedPages],
        blueflame: Option<&mut [MappedPages]>, wr: &mut ibv_send_wr,
    ) -> Result<(), &'static str> {
        if self.state != ibv_qp_state::IBV_QPS_RTS {
            return Err("queue pair cannot send in this state");
        }
        // TODO: the Nautilus driver uses sq.next_wqe
        let mut index = self.sq.head;
        let mut current = Some(wr);
        let mut num_req = 0;
        let memory = self.memory.as_mut().unwrap();
        while current.is_some() {
            let curr = current.take().unwrap();
            // make sure that we're not overflowing
            if self.sq.would_overflow(num_req) {
                return Err("send queue would overflow");
            }
            // check that this work request is not too big
            if u32::try_from(curr.num_sge).unwrap() > self.sq.max_gs {
                return Err("work request has too many sges");
            }
            let ctrl_addr = {
                let ctrl: &mut WqeControlSegment = self.sq.get_element(
                    memory, index,
                )?;
                ctrl.vlan_cv_f_ds = 0.into();
                ctrl.flags = WqeControlSegmentFlags::CQ_UPDATE.bits().into();
                ctrl.flags2 = 0.into();
                VirtAddr::new(
                    ctrl as *mut WqeControlSegment as u64
                )
            };
            let mut wqe_offset = memory.0.offset_of_address(
                ctrl_addr
            ).unwrap();
            wqe_offset += size_of::<WqeControlSegment>();
            let mut wqe_size = size_of::<WqeControlSegment>();
            match self.qp_type {
                ibv_qp_type::IBV_QPT_RC | ibv_qp_type::IBV_QPT_UC => {
                    // extra segments are only required for RDMA
                    if curr.opcode == ibv_wr_opcode::IBV_WR_RDMA_READ
                     || curr.opcode == ibv_wr_opcode::IBV_WR_RDMA_WRITE {
                        let wqe: &mut WqeRemoteAddressSegment = memory.0
                            .as_type_mut(wqe_offset)?;
                        *wqe = WqeRemoteAddressSegment::from_wr(&curr.wr)?;
                        wqe_offset += size_of::<WqeRemoteAddressSegment>();
                        wqe_size += size_of::<WqeRemoteAddressSegment>();
                    }
                },
                ibv_qp_type::IBV_QPT_UD => {
                    let wqe: &mut WqeDatagramSegment = memory.0
                        .as_type_mut(wqe_offset)?;
                    *wqe = WqeDatagramSegment::from_wr(&curr.wr)?;
                    wqe_offset += size_of::<WqeDatagramSegment>();
                    wqe_size += size_of::<WqeDatagramSegment>();
                },
                _ => return Err("invalid queue pair type"),
            }

            // Write data segments in reverse order, so as to overwrite
            // cacheline stamp last within each cacheline. This avoids issues
            // with WQE prefetching.
            wqe_offset += (
                usize::try_from(curr.num_sge).unwrap() - 1
            ) * size_of::<WqeDataSegment>();
            for sge in curr.sg_list.iter().rev() {
                let elem: &mut WqeDataSegment = memory.0
                    .as_type_mut(wqe_offset)?;
                elem.copy_from_sge(sge)?;
                wqe_offset -= size_of::<WqeDataSegment>();
                wqe_size += size_of::<WqeDataSegment>();
            }

            // Possibly overwrite stamping in cacheline with LSO segment
            // only after making sure all data segments are written.
            compiler_fence(Ordering::SeqCst);
            let ctrl: &mut WqeControlSegment = self.sq.get_element(memory, index)?;
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
            let owner = match index & self.sq.wqe_cnt {
                0 => 0,
                _ => 1 << 31,
            };
            ctrl.owner_opcode = (owner | opcode).into();
            // We can improve latency by not stamping the last send queue WQE
            // until after ringing the doorbell, so only stamp here if there are
            // still more WQEs to post.
            if curr.next.is_some() {
                self.sq.stamp_wqe(memory, index + self.sq.spare_wqes.unwrap())?;
            }
            num_req += 1;
            index += 1;
            // TODO: support multiple work requests
            assert!(curr.next.is_none());
        }
        // return if we don't have anything to do
        if num_req == 0 {
            return Ok(());
        }
        // TODO: bf fails for RDMA writes
        if blueflame.is_some() && caps.bf() && num_req == 1 {
            index -= 1;
            let (size, ctrl_address) = {
                let ctrl: &mut WqeControlSegment = self.sq.get_element(
                    memory, index,
                )?;
                ctrl.owner_opcode.set(
                    ctrl.owner_opcode.get() | ((self.sq.head & 0xffff) << 8)
                );
                ctrl.vlan_cv_f_ds.set(
                    ctrl.vlan_cv_f_ds.get() | (self.number << 8)
                );
                let ctrl_address = VirtAddr::new(
                    ctrl as *mut WqeControlSegment as u64
                );
                (ctrl.size().try_into().unwrap(), ctrl_address)
            };
            let ctrl_offset = memory.0.offset_of_address(ctrl_address)
                .ok_or("control segment has invalid address")?;
            // Make sure that descriptor is written to memory
            // before writing to BlueFlame page.
            compiler_fence(Ordering::SeqCst);
            // the UAR determines which BlueFlame page we can use
            // we just use the first register (0..bf_reg_size)
            // each register consists of two buffers (bf_reg_size/2)
            // which we have to alternate between
            let bf_reg: &mut [u64] = blueflame.unwrap()[self.uar_idx].as_slice_mut(
                (index as usize % 2) * (caps.bf_reg_size() / 2),
                caps.bf_reg_size() / 16,
            )?;
            let src = memory.0.as_slice(ctrl_offset, size)?;
            bf_reg[..size].copy_from_slice(src);
            // TODO: will this work when mixing BF and normal sends?
        } else {
            // Make sure that descriptors are written before doorbell.
            compiler_fence(Ordering::SeqCst);
            let doorbell: &mut DoorbellPage = doorbells[self.uar_idx]
                .as_type_mut(0)?;
            doorbell.send_queue_number.write((self.number << 8).into());
        }
        self.sq.stamp_wqe(memory, index + self.sq.spare_wqes.unwrap() - 1)?;
        self.sq.head += num_req;
        Ok(())
    }

    /// Advance the tail of the receive queue.
    /// 
    /// This is called on work completion.
    pub(super) fn advance_receive_queue(&mut self) {
        self.rq.tail += 1;
    }

    /// Advance the tail of the send queue.
    /// 
    /// This is called on work completion.
    pub(super) fn advance_send_queue(&mut self) {
        self.sq.tail += 1;
    }

    /// Destroy this queue pair.
    pub(super) fn destroy(
        mut self, cmd: &mut CommandInterface, caps: &Capabilities,
    ) -> Result<(), &'static str> {
        trace!("destroying QP {}..", self.number);
        if self.state != ibv_qp_state::IBV_QPS_RESET {
            self.modify(cmd, caps, &ibv_qp_attr {
                qp_state: ibv_qp_state::IBV_QPS_RESET,
                ..Default::default()
            }, ibv_qp_attr_mask::IBV_QP_STATE)?;
        }
        // actually free the memory
        self.memory.take().unwrap();
        Ok(())
    }
    
    /// Get the number of this queue pair.
    pub(super) fn number(&self) -> u32 {
        self.number
    }
    
}

impl Drop for QueuePair {
    fn drop(&mut self) {
        if self.memory.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

//#[derive(FromBytes)]
#[repr(C, packed)]
struct QueuePairDoorbell {
    _reserved: u16,
    receive_wqe_index: WriteOnly<U16<BigEndian>>,
}

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
}

impl WorkQueue {
    /// Compute the size of the receive queue and return it.
    fn new_receive_queue(
        hca_caps: &Capabilities, ib_caps: &mut ibv_qp_cap,
    ) -> Result<Self, &'static str> {
        // check the RQ size before proceeding
        if ib_caps.max_recv_wr > 1 << u32::from(hca_caps.log_max_qp_sz()) - IB_SQ_MAX_SPARE
         || ib_caps.max_recv_sge > hca_caps.max_sg_sq().into()
         || ib_caps.max_recv_sge > hca_caps.max_sg_rq().into() {
            return Err("RQ size is invalid")
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
        let wqe_shift = (
            max_gs * u32::try_from(size_of::<WqeDataSegment>()).unwrap()
        ).ilog2();
        let mut max_post = 1 << u32::from(
            hca_caps.log_max_qp_sz()
        ) - IB_SQ_MAX_SPARE;
        if max_post > wqe_cnt {
            max_post = wqe_cnt;
        }
        // update the caps
        ib_caps.max_recv_wr = max_post;
        ib_caps.max_recv_sge = *[
            max_gs, hca_caps.max_sg_sq().into(), hca_caps.max_sg_rq().into(),
        ].iter().min().unwrap();
        Ok(Self {
            wqe_cnt, max_post, max_gs, offset: 0, wqe_shift,
            spare_wqes: None, head: 0, tail: 0,
        })
    }
    
    /// Compute the size of the receive queue and return it.
    fn new_send_queue(
        hca_caps: &Capabilities, ib_caps: &mut ibv_qp_cap,
        qp_type: ibv_qp_type::Type,
    ) -> Result<Self, &'static str> {
        // check the SQ size before proceeding
        if ib_caps.max_send_wr > 1 << u32::from(hca_caps.log_max_qp_sz()) - IB_SQ_MAX_SPARE
         || ib_caps.max_send_sge > hca_caps.max_sg_sq().into()
         || ib_caps.max_send_sge > hca_caps.max_sg_rq().into() {
            return Err("SQ size is invalid")
        }
        let size = ib_caps.max_send_sge * u32::try_from(
            size_of::<WqeDataSegment>()
        ).unwrap() + send_wqe_overhead(qp_type);
        if size > hca_caps.max_desc_sz_sq().into() {
            return Err("SQ size is invalid")
        }
        let wqe_shift = size.next_power_of_two().ilog2();
        // We need to leave 2 KB + 1 WR of headroom in the SQ to allow HW to prefetch.
        let spare_wqes = ib_sq_headroom(wqe_shift);
        let mut wqe_cnt = ib_caps.max_send_wr;
        if wqe_cnt < 256 {
            wqe_cnt = 256;
        }
        wqe_cnt = (wqe_cnt + spare_wqes).next_power_of_two();
        let max_gs = (u32::from(*[
            hca_caps.max_desc_sz_sq(), 1 << wqe_shift
        ].iter().min().unwrap()) - send_wqe_overhead(qp_type)) / u32::try_from(
            size_of::<WqeDataSegment>()
        ).unwrap();
        let max_post = wqe_cnt - spare_wqes;
        // update the caps
        ib_caps.max_send_wr = max_post;
        ib_caps.max_send_sge = *[
            max_gs, hca_caps.max_sg_sq().into(), hca_caps.max_sg_rq().into(),
        ].iter().min().unwrap();
        Ok(Self {
            wqe_cnt, max_post, max_gs, offset: 0, wqe_shift,
            spare_wqes: Some(spare_wqes), head: 0, tail: 0,
        })
    }
    
    /// Get the size.
    fn size(&self) -> u32 {
        self.wqe_cnt << self.wqe_shift
    }
    
    /// Get an element of this work queue.
    /// 
    /// The index wraps around to the beginning.
    fn get_element<'e, T: FromBytes>(
        &self, memory: &'e mut utils::PageToFrameMapping, mut index: u32,
    ) -> Result<&'e mut T, &'static str> {
        // wrap around
        index &= self.wqe_cnt - 1;
        let (pages, _addresss) = memory;
        pages.as_type_mut(
            (self.offset + (index << self.wqe_shift)).try_into().unwrap()
        )
    }

    /// Stamp this WQE so that it is invalid if prefetched by marking the
    /// first four bytes of every 64 byte chunk with 0xffffffff, except for
    /// the very first chunk of the WQE.
    /// 
    /// This is not part of `WqeControlSegment` because we need to access other
    /// parts of the buffer here.
    fn stamp_wqe(
        &mut self, memory: &mut utils::PageToFrameMapping, index: u32,
    ) -> Result<(), &'static str> {
        let (size, ctrl_address) = {
            let ctrl: &mut WqeControlSegment = self.get_element(memory, index)?;
            let ctrl_address = VirtAddr::new(
                ctrl as *mut WqeControlSegment as u64
            );
            (ctrl.size().try_into().unwrap(), ctrl_address)
        };
        let ctrl_offset = memory.0.offset_of_address(ctrl_address)
            .ok_or("control segment has invalid address")?;
        for i in (64..size).step_by(64) {
            let bytes = memory.0.as_slice_mut(ctrl_offset, size)?;
            bytes[i] = u8::MAX;
            bytes[i+1] = u8::MAX;
            bytes[i+2] = u8::MAX;
            bytes[i+3] = u8::MAX;
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
        ibv_qp_type::IBV_QPT_UD => {
            size_of::<WqeControlSegment>() + size_of::<WqeDatagramSegment>()
        },
        ibv_qp_type::IBV_QPT_UC => {
            size_of::<WqeControlSegment>() + size_of::<WqeRemoteAddressSegment>()
        },
        ibv_qp_type::IBV_QPT_RC => {
            size_of::<WqeControlSegment>() /* + size_of::<WqeMaskedAtomicSegment>() */
            + size_of::<WqeRemoteAddressSegment>()
        },
        _ => {
            size_of::<WqeControlSegment>()
        },
    }.try_into().unwrap()
}

#[derive(FromBytes)]
#[repr(C)]
struct WqeControlSegment {
    owner_opcode: U32<BigEndian>,
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
        self.lkey.set(sge.lkey);
        self.addr.set(
            utils::get_physical_address(VirtAddr::new(sge.addr)).as_u64()
        );
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
        Self { byte_count: 0.into(), lkey: INVALID_LKEY.into(), addr: 0.into() }
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
    #[skip] __: B4,
    #[skip(getters)] service_type: u8,
    #[skip] __: B3,
    #[skip(getters)] path_migration_state: B2,
    #[skip] __: B19,
    #[skip(getters)] protection_domain: B24,
    mtu: B3,
    #[skip(getters)] msg_max: B5,
    #[skip] __: bool,
    #[skip(getters)] log_rq_size: B4,
    #[skip(getters)] log_rq_stride: B3,
    #[skip(getters)] sq_no_prefetch: bool,
    #[skip(getters)] log_sq_size: B4,
    #[skip(getters)] log_sq_stride: B3,
    #[skip(getters)] roce_mode: B2,
    #[skip] __: bool,
    #[skip(getters)] reserved_lkey: bool,
    #[skip] __: B12,
    #[skip(getters)] usr_page: B24,
    #[skip] __: u8,
    local_qpn: B24,
    #[skip] __: u8,
    #[skip(getters)] remote_qpn: B24,
    // nested bitfields are only allowed to be 128 bits
    // and nesting bitfields makes them little endian
    #[skip] __: B17,
    #[skip(getters)] primary_disable_pkey_check: bool,
    #[skip] __: B7,
    #[skip(getters)] primary_pkey_index: B7,
    #[skip] __: u8,
    #[skip(getters)] primary_grh: bool,
    #[skip(getters)] primary_mlid: B7,
    #[skip(getters)] primary_rlid: u16,
    #[skip(getters)] primary_ack_timeout: B5,
    #[skip] __: B4,
    #[skip(getters)] primary_mgid_index: B7,
    #[skip] __: u8,
    #[skip(getters)] primary_hop_limit: u8,
    #[skip] __: B4,
    #[skip(getters)] primary_tclass: u8,
    #[skip(getters)] primary_flow_label: B20,
    #[skip(getters)] primary_rgid: u128,
    #[skip(getters)] primary_sched_queue: u8,
    #[skip] __: bool,
    #[skip(getters)] primary_vlan_index: B7,
    #[skip] __: u32,
    #[skip(getters)] primary_dmac: B48,
    #[skip] __: B17,
    #[skip(getters)] alternate_disable_pkey_check: bool,
    #[skip] __: B7,
    #[skip(getters)] alternate_pkey_index: B7,
    #[skip] __: u8,
    #[skip(getters)] alternate_grh: bool,
    #[skip(getters)] alternate_mlid: B7,
    #[skip(getters)] alternate_rlid: u16,
    #[skip(getters)] alternate_ack_timeout: B5,
    #[skip] __: B4,
    #[skip(getters)] alternate_mgid_index: B7,
    #[skip] __: u8,
    #[skip(getters)] alternate_hop_limit: u8,
    #[skip] __: B4,
    #[skip(getters)] alternate_tclass: u8,
    #[skip(getters)] alternate_flow_label: B20,
    #[skip(getters)] alternate_rgid: u128,
    #[skip(getters)] alternate_sched_queue: u8,
    #[skip] __: bool,
    #[skip(getters)] alternate_vlan_index: B7,
    #[skip] __: u32,
    #[skip(getters)] alternate_dmac: B48,
    #[skip] __: u16,
    #[skip(getters)] rnr_retry: B3,
    #[skip] __: B53,
    #[skip(getters)] next_send_psn: B24,
    #[skip] __: u8,
    #[skip(getters)] cqn_send: B24,
    #[skip(getters)] roce_entropy: u16,
    #[skip] __: B56,
    #[skip(getters)] last_acked_psn: B24,
    #[skip] __: u8,
    #[skip(getters)] ssn: B24,
    #[skip] __: u16,
    #[skip(getters)] remote_read: bool,
    #[skip(getters)] remote_write: bool,
    #[skip(getters)] remote_atomic: bool,
    #[skip] __: B16,
    #[skip(getters)] min_rnr_nak: B5,
    #[skip(getters)] next_recv_psn: B24,
    #[skip] __: u16,
    #[skip(getters)] xrcd: u16,
    #[skip] __: u8,
    #[skip(getters)] cqn_receive: B24,
    /// The last three bits must be zero.
    #[skip(getters)] db_record_addr: u64,
    qkey: u32,
    #[skip] __: u8,
    #[skip(getters)] srqn: B24,
    #[skip] __: u8,
    #[skip(getters)] msn: B24,
    rq_wqe_counter: u16,
    sq_wqe_counter: u16,
    // rate_limit_params
    #[skip] __: B56,
    #[skip(getters)] qos_vport: u8,
    #[skip] __: u32,
    #[skip(getters)] num_rmc_peers: u8,
    #[skip(getters)] base_mkey: B24,
    #[skip] __: B2,
    #[skip(getters)] log_page_size: B6,
    #[skip] __: u16,
    /// The last three bits must be zero.
    #[skip(getters)] mtt_base_addr: B40,
    #[skip] __: u128,
    #[skip] __: u128,
    #[skip] __: u64,
}

impl core::fmt::Debug for QueuePairContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f
            .debug_struct("QueuePairContext")
            .field("state", &self.state())
            .field("MTU", &ibv_mtu::from_repr(self.mtu()))
            .field("QKEY", &self.qkey())
            .field("QP Number", &self.local_qpn())
            .field("Send Counter", &self.sq_wqe_counter())
            .field("Receive Counter", &self.rq_wqe_counter())
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
