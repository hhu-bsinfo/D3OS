//! This module consists of functions that create, work with and destroy
//! completion queues. Furthermore its functions can consume and print
//! completion queue elements.

use core::{
    mem::size_of,
    sync::atomic::{compiler_fence, Ordering},
};

use log::trace;
use modular_bitfield_msb::{
    bitfield,
    specifiers::{B2, B24, B3, B40, B48, B5, B6},
};
use tock_registers::{interfaces::Writeable, registers::WriteOnly};
use x86_64::structures::paging::{Page, Size4KiB};

use crate::process_manager;

use super::{
    cmd::{CommandInterface, InputParam, OutputParam, Opcode},
    device::{uar_index_to_hw, PAGE_SHIFT},
    fw::DoorbellPage,
    icm::ICM_PAGE_SHIFT,
    ConnectX3Nic,
};

/// Size in bytes of a hardware completion queue entry. CX3 also supports a 64 B format, but this
/// driver always uses the 32 B one (matches the layout `os/library/ibverbs/src/mlx4/completion_queue.rs`
/// parses).
const CQE_SIZE: usize = 32;

#[derive(Debug)]
pub(super) struct CompletionQueue {
    number: u32,
    uar_idx: usize,
    uar_page: Page<Size4KiB>,
    // TODO: deallocate mtt properly, see the equivalent TODO on `queue_pair::QueuePair`.
    mtt: Option<u64>,
    arm_sequence_number: u32,
    consumer_index: u32,
    // TODO: bind the lifetime to the one of the event queue
    eq_number: Option<usize>,
}

impl CompletionQueue {
    /// Create a new completion queue over a userspace-owned, -mmap'd `buffer` and
    /// `doorbell_ptr` (a two-word `CompletionQueueDoorbell` record). The kernel only builds the
    /// MTT for the buffer and runs the CMD-interface transition; polling and CQE parsing happen
    /// entirely in userspace against the mapped memory from here on.
    pub(super) fn new(dev: &mut ConnectX3Nic, num_entries: u32, buffer: *const u8, doorbell_ptr: *const u64) -> Result<Self, &'static str> {
        let number: u32 = dev.offsets.alloc_cqn().try_into().unwrap();
        let uar_idx = dev.offsets.alloc_uar();

        let process = process_manager().read().current_process();
        let uar_page = dev.map_uar(uar_idx, &process, alloc::format!("cq-uar-{uar_idx}").as_str())?;

        assert_eq!(buffer.addr() % crate::memory::PAGE_SIZE, 0, "CQE buffer is not page aligned");
        let size = usize::try_from(num_entries).unwrap() * CQE_SIZE;
        let start: Page<Size4KiB> = Page::containing_address(x86_64::VirtAddr::from_ptr(buffer));
        let end = start + u64::try_from(size.next_multiple_of(crate::memory::PAGE_SIZE) / crate::memory::PAGE_SIZE).unwrap();
        let mtt = dev.icm_tables.memory_regions().alloc_mtt_for_pages(&dev.capabilities, Page::range(start, end))?;

        let doorbell_address = process.virtual_address_space
            .get_phys(doorbell_ptr as u64)
            .ok_or("doorbell not mapped to physical address")?;

        let arm_sequence_number = 1;
        let consumer_index = 0;

        let mut ctx = CompletionQueueContext::new();
        ctx.set_log_size(num_entries.ilog2().try_into().unwrap());
        ctx.set_usr_page(uar_index_to_hw(uar_idx).try_into().unwrap());
        let mut eq_number = None;
        if let Some(eq) = dev.eqs.get(0) {
            ctx.set_comp_eqn(eq.number().try_into().unwrap());
            eq_number = Some(eq.number());
        }
        ctx.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
        ctx.set_mtt_base_addr(mtt);
        ctx.set_doorbell_record_addr(doorbell_address.as_u64());
        dev.cmd.execute_command(Opcode::Sw2HwCq, None, InputParam::Mailbox(&ctx.bytes), Some(number.try_into().unwrap()), OutputParam::Empty)?;

        let cq = Self {
            number,
            uar_idx,
            uar_page,
            mtt: Some(mtt),
            arm_sequence_number,
            consumer_index,
            eq_number,
        };
        trace!("created new CQ: {:?}", cq);
        Ok(cq)
    }

    /// The UAR page mapped into the calling process, for userspace to ring the arm doorbell
    /// from directly.
    pub(super) fn uar_page_ptr(&self) -> *mut u8 {
        self.uar_page.start_address().as_mut_ptr()
    }

    /// Destroy this completion queue.
    pub(super) fn destroy(mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        // TODO: should make sure to undo all card state tied to this CQ
        cmd.execute_command(Opcode::Hw2SwCq, None, InputParam::Empty, Some(self.number.try_into().unwrap()), OutputParam::Empty)?;
        // TODO: deallocate mtt properly
        let _ = self.mtt.take();
        Ok(())
    }

    /// Arm this completion queue by writing the consumer index to the appropriate doorbell.
    ///
    /// This is one-time control-path work done at creation, so it stays in the kernel: the
    /// event-driven (as opposed to polling) completion model this would support isn't used by
    /// this driver, but leaving the initial arm here matches what the reference driver does and
    /// costs nothing on the (userspace) polling hot path.
    pub(super) fn arm(&mut self, doorbell_ptr: *mut u64, doorbells: &mut [DoorbellPage]) -> Result<(), &'static str> {
        const _DOORBELL_REQUEST_NOTIFICATION_SOLICITED: u32 = 0x1;
        const DOORBELL_REQUEST_NOTIFICATION: u32 = 0x2;
        let sn = self.arm_sequence_number & 3;
        let ci = self.consumer_index & 0xffffff;
        let cmd = DOORBELL_REQUEST_NOTIFICATION;
        let doorbell_record = unsafe { &mut *doorbell_ptr.cast::<CompletionQueueDoorbell>() };
        doorbell_record.arm_consumer_index.set((sn << 28 | cmd << 24 | ci).to_be());
        // Make sure that the doorbell record in host memory is
        // written before ringing the doorbell via PCI MMIO.
        compiler_fence(Ordering::SeqCst);
        let doorbell: &mut DoorbellPage = &mut doorbells[self.uar_idx];
        doorbell.cq_sn_cmd_num.set((sn << 28 | cmd << 24 | self.number).to_be());
        doorbell.cq_consumer_index.set(ci.to_be());
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
    #[skip]
    flags: u32,
    #[skip]
    __: B48,
    #[skip]
    page_offset: u16,
    #[skip]
    __: B3,
    #[skip(getters)]
    log_size: B5,
    #[skip(getters)]
    usr_page: B24,
    #[skip]
    cq_period: u16,
    #[skip]
    cq_max_count: u16,
    #[skip]
    __: B24,
    #[skip(getters)]
    comp_eqn: u8,
    #[skip]
    __: B2,
    #[skip(getters)]
    log_page_size: B6,
    #[skip]
    __: u16,
    // the last three bits must be zero
    #[skip(getters)]
    mtt_base_addr: B40,
    #[skip]
    __: u8,
    #[skip]
    last_notified_index: B24,
    #[skip]
    __: u8,
    #[skip]
    solicit_producer_index: B24,
    #[skip]
    __: u8,
    #[skip]
    consumer_index: B24,
    #[skip]
    __: u8,
    #[skip]
    producer_index: B24,
    #[skip]
    __: u64,
    // the last three bits must be zero
    #[skip(getters)]
    doorbell_record_addr: u64,
}

/// The doorbell record layout, kept here since the kernel still writes the initial
/// `arm_consumer_index` at CQ creation (see [`CompletionQueue::arm`]). CQE parsing itself
/// (including this record's `update_consumer_index` half) moved to
/// `os/library/ibverbs/src/mlx4/completion_queue.rs`.
// PRM: "CQ DoorBell Records are aligned on an 8B boundary."
#[repr(C, align(8))]
struct CompletionQueueDoorbell {
    update_consumer_index: WriteOnly<u32>,
    arm_consumer_index: WriteOnly<u32>,
}
