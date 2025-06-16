//! This module consists of functions that create, work with and destroy
//! completion queues. Furthermore its functions can consume and print
//! completion queue elements.

use core::{mem::size_of, sync::atomic::{compiler_fence, Ordering}};

use byteorder::BigEndian;
use crate::infiniband::ib_core::{ibv_wc, ibv_wc_flags, ibv_wc_opcode, ibv_wc_status};
use modular_bitfield_msb::{bitfield, prelude::{B12, B4, B7}, specifiers::{B2, B24, B3, B40, B48, B5, B6}};
use strum_macros::FromRepr;
use volatile::WriteOnly;
use zerocopy::{U32};
use log::{trace, warn, error};
use super::queue_pair::{QueuePair, QueuePairOpcode};
use super::utils;
use super::utils::{MappedPages, PageToFrameMapping};
use crate::memory::{PAGE_SIZE};

use super::{
    cmd::{CommandInterface, Opcode},
    device::{uar_index_to_hw, PAGE_SHIFT},
    event_queue::EventQueue,
    fw::{Capabilities, DoorbellPage},
    icm::{MrTable, ICM_PAGE_SHIFT},
    Offsets
};

#[derive(Debug)]
pub(super) struct CompletionQueue {
    number: u32,
    num_entries: u32,
    memory: Option<PageToFrameMapping>,
    uar_idx: usize,
    doorbell_page: MappedPages,
    // TODO: somehow free this on Drop
    _mtt: u64,
    arm_sequence_number: u32,
    consumer_index: u32,
    // TODO: bind the lifetime to the one of the event queue
    eq_number: Option<usize>,
}

impl CompletionQueue {
    /// Create a new completion queue.
    /// 
    /// This is quite like creating an event queue.
    pub(super) fn new(
        cmd: &mut CommandInterface, caps: &Capabilities, offsets: &mut Offsets,
        memory_regions: &mut MrTable, eq: Option<&EventQueue>,
        num_entries: u32,
    ) -> Result<Self, &'static str> {
        let number: u32 = offsets.alloc_cqn().try_into().unwrap();
        let uar_idx = offsets.alloc_scq_db();
        let num_pages = (
            usize::try_from(num_entries).unwrap() * size_of::<CompletionQueueEntry>()
        ).next_multiple_of(PAGE_SIZE) / PAGE_SIZE;
        
        let mut operation_container = utils::Operations::default();
        let size = num_pages * PAGE_SIZE + size_of::<CompletionQueueEntry>() - 1;
        let mapped_page_to_frame = utils::create_cont_mapping_with_dma_flags(
            utils::pages_required(size))?.fetch_in_addr()?;

        let bytes = utils::start_page_as_mut_ptr::<u8>(mapped_page_to_frame.0.into_range().start);
        
        operation_container.create_fill(&(0u8, bytes, size));
        operation_container.perform_and_flush();

        let mtt = memory_regions.alloc_mtt(cmd, caps, num_pages, 
            mapped_page_to_frame.1)?;
        let (mut doorbell_page, doorbell_address) = 
            utils::create_cont_mapping_with_dma_flags(
                utils::pages_required(size_of::<CompletionQueueDoorbell>()))
                ?.fetch_in_addr()?;
        let doorbell: &mut CompletionQueueDoorbell = doorbell_page.as_type_mut(0)?;
        doorbell.update_consumer_index.write(0.into());
        doorbell.arm_consumer_index.write(0.into());
        let arm_sequence_number = 1;
        let consumer_index = 0;

        let mut ctx = CompletionQueueContext::new();
        ctx.set_log_size(num_entries.ilog2().try_into().unwrap());
        ctx.set_usr_page(uar_index_to_hw(uar_idx).try_into().unwrap());
        let mut eq_number = None;
        if let Some(eq) = eq {
            ctx.set_comp_eqn(eq.number().try_into().unwrap());
            eq_number = Some(eq.number());
        }
        ctx.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
        ctx.set_mtt_base_addr(mtt);
        ctx.set_doorbell_record_addr(doorbell_address.as_u64());
        let _ : () = cmd.execute_command(
            Opcode::Sw2HwCq, (), &ctx.bytes[..], number.try_into().unwrap(),
        )?;

        let cq = Self {
            number, num_entries, memory: Some(mapped_page_to_frame), uar_idx, doorbell_page,
            _mtt: mtt, arm_sequence_number, consumer_index, eq_number,
        };
        trace!("created new CQ: {:?}", cq);
        Ok(cq)
    }

    /// Destroy this completion queue.
    pub(super) fn destroy(
        mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        // TODO: should make sure to undo all card state tied to this CQ
        cmd.execute_command::<_, _, ()>(
            Opcode::Hw2SwCq, (), (), self.number.try_into().unwrap(),
        )?;
        // actually free the mememory
        self.memory.take().unwrap();
        Ok(())
    }

    /// Arm this completion queue by writing the consumer index to the
    /// appropriate doorbell.
    pub(super) fn arm(
        &mut self, doorbells: &mut [MappedPages],
    ) -> Result<(), &'static str> {
        const _DOORBELL_REQUEST_NOTIFICATION_SOLICITED: u32 = 0x1;
        const DOORBELL_REQUEST_NOTIFICATION: u32 = 0x2;
        let sn = self.arm_sequence_number & 3;
        let ci = self.consumer_index & 0xffffff;
        let cmd = DOORBELL_REQUEST_NOTIFICATION;
        let doorbell_record: &mut CompletionQueueDoorbell = self.doorbell_page
            .as_type_mut(0)?;
        doorbell_record.arm_consumer_index.write(
            (sn << 28 | cmd << 24 | ci).into()
        );
        // Make sure that the doorbell record in host memory is
        // written before ringing the doorbell via PCI MMIO.
        compiler_fence(Ordering::SeqCst);
        let doorbell: &mut DoorbellPage = doorbells[self.uar_idx]
            .as_type_mut(0)?;
        doorbell.cq_sn_cmd_num.write(
            (sn << 28 | cmd << 24 | u32::try_from(self.number).unwrap()).into()
        );
        doorbell.cq_consumer_index.write(ci.into());
        Ok(())
    }

    /// Query this completion queue for debugging purposes.
    pub(super) fn query(
        &mut self, cmd: &mut CommandInterface,
    ) -> Result<(), &'static str> {
        let bytes: MappedPages = cmd.execute_command(
            Opcode::QueryCq, (), (), self.number,
        )?;
        let ctx = CompletionQueueContext::from_bytes(
            bytes.as_slice(0, size_of::<CompletionQueueContext>())?
                .try_into().unwrap()
        );
        trace!("current CQ state: {ctx:?}");
        Ok(())
    }

    /// Poll this completion queue and return the number of new completions.
    /// 
    /// This is used by ibv_poll_cq.
    pub(super) fn poll(
        &mut self, eqs: &mut [EventQueue], qps: &mut [QueuePair],
        doorbells: &mut [MappedPages], wc: &mut [ibv_wc],
    ) -> Result<usize, &'static str> {
        // try to poll the assiociated event queue first
        if let Some(eq_number) = self.eq_number {
            eqs.iter_mut()
                .find(|eq| eq.number() == eq_number)
                .ok_or("invalid event queue number")?
                .handle_events(doorbells)?;
        }
        let mut completions = 0;
        // poll one for as long as there are elements
        while completions < wc.len() {
            if self.poll_one(qps, &mut wc[completions])? {
                completions += 1;
            } else {
                break;
            }
        }
        let doorbell_record: &mut CompletionQueueDoorbell = self.doorbell_page
            .as_type_mut(0)?;
        doorbell_record.update_consumer_index.write(
            (self.consumer_index & 0xffffff).into()
        );
        Ok(completions)
    }

    /// Poll this completion queue for one work completion.
    /// 
    /// Return true if there are more.
    #[allow(unreachable_patterns)]
    fn poll_one(
        &mut self, qps: &mut [QueuePair], wc: &mut ibv_wc,
    ) -> Result<bool, &'static str> {
        const CQE_OPCODE_ERROR: u8 = 0x1e;
        const _CQE_OPCODE_RESIZE: u8 = 0x16;
        // clear the wc first
        *wc = ibv_wc::default();
        if let Some(cqe) = self.get_next_cqe_sw()? {
            self.consumer_index += 1;
            // Make sure we read CQ entry contents after we've checked the
            // ownership bit.
            compiler_fence(Ordering::SeqCst);
            wc.qp_num = cqe.qp_number();
            if let Some(qp) = qps.iter_mut()
                .find(|qp| qp.number() == cqe.qp_number()) {
                if cqe.is_send() {
                    qp.advance_send_queue();
                } else {
                    qp.advance_receive_queue();
                }
            } else {
                warn!("completion has invalid queue pair number {}", cqe.qp_number());
            }
            if cqe.opcode() == CQE_OPCODE_ERROR {
                let checksum_bytes = cqe.checksum().to_be_bytes();
                let vendor_err_syndrome = checksum_bytes[0];
                let syndrome = Syndrome::from_repr(checksum_bytes[1])
                    .ok_or("invalid error syndrome")?;
                error!(
                    "work completion error: (QPN {}, WQE index {}, vendor syndrome {}, syndrome {:?}, opcode {})",
                    cqe.qp_number(), cqe.wqe_index(), vendor_err_syndrome,
                    syndrome, cqe.opcode(),
                );
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
                let opcode = QueuePairOpcode::from_repr(cqe.opcode().into())
                    .ok_or("invalid opcode")?;
                match opcode {
                    QueuePairOpcode::RdmaWrite => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_WRITE;
                    },
                    QueuePairOpcode::RdmaWriteImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_WRITE;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                    },
                    QueuePairOpcode::Send => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                    },
                    QueuePairOpcode::SendImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                    },
                    QueuePairOpcode::SendInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_SEND;
                    },
                    QueuePairOpcode::RdmaRead => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RDMA_READ;
                        wc.byte_len = cqe.byte_cnt();
                    },
                    QueuePairOpcode::AtomicCs | QueuePairOpcode::MaskedAtomicCs => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_COMP_SWAP;
                        wc.byte_len = 8;
                    },
                    QueuePairOpcode::AtomicFa | QueuePairOpcode::MaskedAtomicFa => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_FETCH_ADD;
                        wc.byte_len = 8;
                    },
                    QueuePairOpcode::LocalInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_LOCAL_INV;
                    },
                    _ => {},
                }
            } else {
                let opcode = ReceiveOpcode::from_repr(cqe.opcode().into())
                    .ok_or("invalid opcode")?;
                wc.byte_len = cqe.byte_cnt();
                match opcode {
                    ReceiveOpcode::RdmaWriteImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV_RDMA_WITH_IMM;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    },
                    ReceiveOpcode::SendInval => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_INV);
                        todo!("set invalidate_rkey");
                    },
                    ReceiveOpcode::Send => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                    },
                    ReceiveOpcode::SendImm => {
                        wc.opcode = ibv_wc_opcode::IBV_WC_RECV;
                        wc.wc_flags.insert(ibv_wc_flags::IBV_WC_WITH_IMM);
                        wc.imm_data = cqe.immed_rss_invalid();
                    },
                }
                wc.src_qp = cqe.rqpn();
                wc.dlid_path_bits = cqe.mlpath();
                if cqe.g() {
                    wc.wc_flags.insert(ibv_wc_flags::IBV_WC_GRH);
                }
                wc.pkey_index = (cqe.immed_rss_invalid() & 0x7f)
                    .try_into().unwrap();
                wc.slid = cqe.slid();
                wc.sl = cqe.sl();
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the next element.
    fn get_next_cqe_sw(&mut self) -> Result<Option<CompletionQueueEntry>, &'static str> {
        let index = self.consumer_index;
        // get the cqe
        let cqe_bytes: &[u8] = self.memory.as_mut().unwrap().0.as_slice(
            (
                // wrap around
                usize::try_from(index & (self.num_entries - 1)).unwrap()
            ) * size_of::<CompletionQueueEntry>(),
            size_of::<CompletionQueueEntry>(),
        )?;
        let cqe = CompletionQueueEntry::from_bytes(
            cqe_bytes.try_into().unwrap()
        );
        // check if it's valid
        // the ownership bit is flipping every round
        if cqe.owner() ^ ((index & self.num_entries) != 0) {
            Ok(None)
        } else {
            Ok(Some(cqe))
        }
    }

    /// Get the number of this completion queue.
    pub(super) fn number(&self) -> u32 {
        self.number
    }
}

impl Drop for CompletionQueue {
    fn drop(&mut self) {
        if self.memory.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

#[bitfield]
#[derive(Debug)]
#[allow(dead_code)]
struct CompletionQueueContext {
    #[skip] flags: u32,
    #[skip] __: B48,
    #[skip] page_offset: u16,
    #[skip] __: B3,
    #[skip(getters)] log_size: B5,
    #[skip(getters)] usr_page: B24,
    #[skip] cq_period: u16,
    #[skip] cq_max_count: u16,
    #[skip] __: B24,
    #[skip(getters)] comp_eqn: u8,
    #[skip] __: B2,
    #[skip(getters)] log_page_size: B6,
    #[skip] __: u16,
    // the last three bits must be zero
    #[skip(getters)] mtt_base_addr: B40,
    #[skip] __: u8,
    #[skip] last_notified_index: B24,
    #[skip] __: u8,
    #[skip] solicit_producer_index: B24,
    #[skip] __: u8,
    #[skip] consumer_index: B24,
    #[skip] __: u8,
    #[skip] producer_index: B24,
    #[skip] __: u64,
    // the last three bits must be zero
    #[skip(getters)] doorbell_record_addr: u64,
}

//#[derive(FromBytes)]
#[repr(C, packed)]
struct CompletionQueueDoorbell {
    update_consumer_index: WriteOnly<U32<BigEndian>>,
    arm_consumer_index: WriteOnly<U32<BigEndian>>,
}

// CQE size is 32. There is 64 B support also available in CX3.
#[bitfield(bytes = 32)]
#[derive(Debug)]
struct CompletionQueueEntry {
    #[skip] __: u8,
    qp_number: B24,
    immed_rss_invalid: u32,
    g: bool,
    mlpath: B7,
    rqpn: B24,
    sl: B4,
    #[skip] vid: B12,
    slid: u16,
    #[skip] __: u32,
    byte_cnt: u32,
    wqe_index: u16,
    /// vendor_err_syndrome (u8) and syndrome (u8) on error
    checksum: u16,
    #[skip] __: B24,
    owner: bool,
    is_send: bool,
    #[skip] __: bool,
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
