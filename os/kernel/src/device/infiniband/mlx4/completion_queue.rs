//! This module consists of functions that create, work with and destroy
//! completion queues. Furthermore its functions can consume and print
//! completion queue elements.

use crate::process::process::Process;
use crate::process_manager;
use alloc::sync::Arc;
use core::mem::{size_of, size_of_val_raw};
use core::ops::Div;
use log::{error, trace};
use modular_bitfield_msb::{bitfield, prelude::*};
use uuid::Uuid;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, Size4KiB};

use super::{
    Mlx4Device,
    cmd::{CommandInterface, InputParam, Opcode, OutputParam},
    device::{PAGE_SHIFT, uar_index_to_hw},
    icm::ICM_PAGE_SHIFT,
};

/// Size in bytes of a hardware completion queue entry. CX3 also supports a 64 B format, but this
/// driver always uses the 32 B one (matches the layout `os/library/ibverbs/src/mlx4/cq`
/// parses).
const CQE_SIZE: usize = 32;

#[derive(Debug)]
pub(super) struct CompletionQueue {
    number: u32,
    owner: Uuid,
    // TODO: deallocate mtt properly, see the equivalent TODO on `queue_pair::QueuePair`.
    mtt: Option<u64>,
    // TODO: bind the lifetime to the one of the event queue
    eq_number: Option<usize>,
}

impl CompletionQueue {
    /// Create a new completion queue over a user-owned `buffer` and `doorbell_ptr`
    /// (a two-word consumer-index/arm-index doorbell record). The kernel builds the MTT
    /// for the buffer and transitions ownership of the CQ to the HCA.
    /// All CQs are registered to the first EQ of the device.
    pub(super) fn new(
        dev: &mut Mlx4Device, process: Arc<Process>, num_entries: u32, buffer: *const u8, doorbell_ptr: *const u64, uar_idx: u32,
    ) -> Result<Self, &'static str> {
        if !process.virtual_address_space.access_ok(VirtAddr::from_ptr(doorbell_ptr), size_of::<u64>()) {
            return Err("User has no access to Doorbell");
        }

        if !num_entries.is_power_of_two() {
            error!("invalid CQE count: {}", num_entries);
            return Err("The number off CQE is not a power of 2");
        }

        if num_entries > (1 << 22) {
            return Err("Too many CQEs");
        }

        let number: u32 = dev.offsets.alloc_cqn().try_into().unwrap();
        let log2num_entries = num_entries.ilog2() as u8;

        let buffer_addr = VirtAddr::from_ptr(buffer);
        // The buffer must be aligned to the cqe_stride set in HCA_INIT
        if !buffer_addr.is_aligned(size_of::<u32>() as u64) {
            return Err("Buffer is not aligned to CQE stride");
        }
        let buffer_size = num_entries as usize * CQE_SIZE;
        let start: Page<Size4KiB> = Page::containing_address(buffer_addr);
        let end = start + (buffer_size as u64).div_ceil(start.size());
        let mtt = dev
            .icm_tables
            .memory_regions()
            .alloc_mtt_for_pages(&dev.capabilities, Page::range(start, end))?;

        let doorbell_address = process
            .virtual_address_space
            .get_phys(doorbell_ptr as u64)
            .ok_or("doorbell not mapped to physical address")?;

        let eq_number = dev.eqs.get(0).map(|eq| eq.read().number());

        let mut ctx = CompletionQueueContext::new();
        ctx.set_page_offset(buffer_addr.page_offset().into());
        ctx.set_log_size(log2num_entries);
        ctx.set_usr_page(uar_index_to_hw(uar_idx).try_into().unwrap());
        if let Some(eqn) = eq_number {
            ctx.set_comp_eqn(eqn as u8);
        }
        ctx.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
        ctx.set_mtt_base_addr(mtt);
        ctx.set_doorbell_record_addr(doorbell_address.as_u64());
        dev.cmd.execute_command(
            Opcode::Sw2HwCq,
            None,
            InputParam::Mailbox(&ctx.bytes),
            Some(number.try_into().unwrap()),
            OutputParam::Empty,
        )?;

        let cq = Self {
            number,
            owner: process.id(),
            mtt: Some(mtt),
            eq_number,
        };
        trace!("created new CQ: {:?}", cq);
        Ok(cq)
    }

    /// The process that created this completion queue, i.e. the only process allowed to bind a
    /// queue pair to it.
    pub(super) fn owner(&self) -> Uuid {
        self.owner
    }

    /// Destroy this completion queue.
    pub(super) fn destroy(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        // TODO: should make sure to undo all card state tied to this CQ
        cmd.execute_command(
            Opcode::Hw2SwCq,
            None,
            InputParam::Empty,
            Some(self.number.try_into().unwrap()),
            OutputParam::Empty,
        )?;
        // TODO: deallocate mtt properly
        let _ = self.mtt.take();
        Ok(())
    }

    /// Query this completion queue for debugging purposes.
    pub(super) fn query(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        cmd.execute_command(Opcode::QueryCq, None, InputParam::Empty, Some(self.number), OutputParam::Mailbox)?;
        let ctx_bytes: &[u8; size_of::<CompletionQueueContext>()] = unsafe { cmd.output_mailbox_as_ref() };
        let ctx = CompletionQueueContext::from_bytes(*ctx_bytes);
        trace!("current CQ state: {ctx:?}");
        Ok(())
    }

    /// Get the number of this completion queue.
    pub(super) fn number(&self) -> u32 {
        self.number
    }
}

impl Drop for CompletionQueue {
    fn drop(&mut self) {
        if self.mtt.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

#[bitfield]
#[derive(Debug)]
#[allow(dead_code)]
struct CompletionQueueContext {
    // 0x00
    #[skip]
    flags: B32,
    // 0x00
    #[skip]
    __: B32,
    // 0x08
    #[skip]
    __: B16,
    /// Offset to buffer from the beginning of the first page defined by MTT. Bits 4:0 have to be zero
    page_offset: B16,
    // 0x0C
    #[skip]
    __: B3,
    #[skip(getters)]
    log_size: B5,
    #[skip(getters)]
    usr_page: B24,
    // 0x10
    #[skip]
    cq_period: B16,
    #[skip]
    cq_max_count: B16,
    // 0x14
    #[skip]
    __: B24,
    #[skip(getters)]
    comp_eqn: u8,
    // 0x18
    #[skip]
    __: B2,
    #[skip(getters)]
    log_page_size: B6,
    #[skip]
    __: u16,
    // the last three bits must be zero
    #[skip(getters)]
    mtt_base_addr: B40,
    // 0x20
    #[skip]
    __: u8,
    #[skip]
    last_notified_index: B24,
    // 0x24
    #[skip]
    __: u8,
    #[skip]
    solicit_producer_index: B24,
    // 0x28
    #[skip]
    __: u8,
    #[skip]
    consumer_index: B24,
    // 0x2C
    #[skip]
    __: u8,
    #[skip]
    producer_index: B24,
    // 0x30
    #[skip]
    __: u64,
    // 0x38
    // the last three bits must be zero
    doorbell_record_addr: u64,
}
