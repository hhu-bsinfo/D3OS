//! This module consists of functions that create, work with and destroy event queues.
//! Additionally it holds the interrupt handling function to consume EQEs.

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::{
    mem::size_of,
    sync::atomic::{compiler_fence, Ordering},
};

use super::{device_handle_to_idx, get_dev_list, utils, DEV_LIST};
use super::utils::MappedPages;
use crate::memory::PAGE_SIZE;
use alloc::vec::Vec;
use core::ptr::eq;
use core::sync::atomic::AtomicUsize;
use bitflags::bitflags;
use byteorder::BigEndian;
use log::{debug, error, trace, warn};
use zerocopy::U32;
use modular_bitfield_msb::{
    bitfield,
    specifiers::{B10, B16, B2, B22, B24, B4, B40, B5, B6, B60, B7, B72, B96},
};
use spin::{Mutex, RwLock};
use strum_macros::FromRepr;
use tock_registers::interfaces::Writeable;
use tock_registers::registers::WriteOnly;
use x86_64::structures::paging::Page;
use x86_64::VirtAddr;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::interrupt_dispatcher;
use super::{
    cmd::{CommandInterface, InputParam, Opcode, OutputParam},
    device::PAGE_SHIFT,
    fw::{Capabilities, DoorbellPage},
    icm::{MrTable, ICM_PAGE_SHIFT},
    Offsets,
};

const _NUM_ASYNC_EQE: u32 = 0x100;
const NUM_SPARE_EQE: u32 = 0x80;

/// Initialize the event queues.
/// This creates all of the EQs ahead of time,
/// passes their ownership to the hardware and calls MapEq.
pub(super) fn init_eqs(
    cmd: &mut CommandInterface, doorbell_pages: &[DoorbellPage], caps: &Capabilities, offsets: &mut Offsets, memory_regions: &mut MrTable,
    clr_int: ClrInt,
) -> Result<Vec<Arc<RwLock<EventQueue>>>, &'static str> {
    const NUM_EQS: usize = 1;
    let mut eqs = Vec::with_capacity(NUM_EQS);
    for i in 0..NUM_EQS {
        // four EQE doorbells per page;
        let doorbell_page = Page::from_start_address(VirtAddr::from_ptr(&doorbell_pages[i/4] as *const _)).expect("Doorbell not aligned");
        // TODO: use interrupts here
        let eq = EventQueue::new(cmd, caps, offsets, memory_regions, doorbell_page, None)?;
        eqs.push(Arc::new(RwLock::new(eq)));
    }

    // map all events to the first (and only) event queue
    interrupt_dispatcher().assign(InterruptVector::Free3, Box::new(EventQueueHandler::new(eqs[0].clone(), clr_int)));
    eqs[0].write().map_all_events(cmd)?;
    eqs[0].read().ring(true);
    Ok(eqs)
}

/// Where the card's legacy-interrupt clear register lives, and the value that clears it.
///
/// Ringing an EQ's doorbell (even with the arm bit set) only updates that EQ's own
/// software-visible arm state; it does not touch the physical, level-triggered legacy (INTx)
/// interrupt line. Per the PRM: "To clear an interrupt, the driver should write the value
/// (1<<intapin) into the clr_int register." Without this write the line stays asserted forever
/// once anything has posted an event, so the CPU keeps re-entering the handler indefinitely even
/// though `poll_one` finds nothing left to consume.
#[derive(Clone, Copy)]
pub(super) struct ClrInt {
    regs: MappedPages,
    offset: usize,
    mask: u64,
}

impl ClrInt {
    pub(super) fn new(regs: MappedPages, offset: u64, inta_pin: u8) -> Self {
        Self { regs, offset: offset as usize, mask: 1 << inta_pin }
    }

    fn clear(&self) {
        unsafe {
            self.regs.page_range().start.start_address()
                .as_mut_ptr::<u8>()
                .add(self.offset)
                .cast::<u64>()
                .write_volatile(self.mask.to_be())
        }
    }
}

#[derive(Debug)]
pub(super) struct EventQueue {
    number: usize,
    num_entries: u32,
    memory: Option<utils::PageToFrameMapping>,
    doorbell_page: Page,
    // TODO: somehow free this on Drop
    _mtt: u64,
    consumer_index: Mutex<u32>,
    /// IRQ number on bus
    intr_vector: Option<u8>,
    /// IRQ we will see
    base_vector: Option<u8>,
    /// event bitmask
    async_ev_mask: AsyncEventMask,
}

#[repr(u8)]
enum EventQueueState {
    Armed = 0x9,
    Fired = 0xa,
    AlwaysArmed = 0xb,
}

impl EventQueue {
    // Create a new event queue. If `base_vector` is given, it will be interrupt
    // driven, else it will be polled.
    fn new(
        cmd: &mut CommandInterface, caps: &Capabilities, offsets: &mut Offsets, memory_regions: &mut MrTable, doorbell_page: Page, base_vector: Option<u8>,
    ) -> Result<Self, &'static str> {
        // EQE size is 32. There is 64 B support also available in CX3.
        let number = offsets.alloc_eqn();
        let num_entries: u32 = 4096; // NUM_ASYNC_EQE + NUM_SPARE_EQE
        let num_pages = (num_entries as usize * size_of::<EventQueueEntry>()).div_ceil(PAGE_SIZE);

        let mut mapped_page_to_frame = utils::create_cont_mapping_with_dma_flags(num_pages)?.fetch_in_addr()?;
        // Invalidate all EQEs
        mapped_page_to_frame.0.as_bytes_mut().fill(0);

        let mtt = memory_regions.alloc_mtt_for_pages(caps, mapped_page_to_frame.0.page_range())?;
        // TODO: register interrupt correctly
        // TODO: Should use MSI-X instead of legacy INTs
        let intr_vector = base_vector.and_then(|_| todo!());

        let mut ctx = EventQueueContext::new();
        ctx.set_state(if base_vector.is_some() { EventQueueState::Armed } else { EventQueueState::Fired } as u8);
        ctx.set_log_eq_size(num_entries.ilog2().try_into().unwrap());
        if let Some(base_vector) = base_vector {
            ctx.set_intr(base_vector.try_into().unwrap());
        }
        ctx.set_log_page_size(PAGE_SHIFT - ICM_PAGE_SHIFT);
        ctx.set_mtt_base_addr(mtt);
        cmd.execute_command(Opcode::Sw2HwEq, None, InputParam::Mailbox(&ctx.bytes), Some(number.try_into().unwrap()), OutputParam::Empty)?;

        let async_ev_mask = AsyncEventMask::empty();
        let eq = Self {
            number,
            num_entries,
            memory: Some(mapped_page_to_frame),
            doorbell_page,
            _mtt: mtt,
            consumer_index: Mutex::new(0),
            intr_vector,
            base_vector,
            async_ev_mask,
        };
        trace!("created new EQ: {:?}", eq);
        Ok(eq)
    }

    /// Map all event types to this EQ.
    // TODO: should parameterize the types of events given to this EQ
    fn map_all_events(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        // TODO: unmask IRQ
        self.async_ev_mask = AsyncEventMask::all();
        let unmap = false;
        cmd.execute_command(
            Opcode::MapEq,
            None,
            InputParam::Immediate(self.async_ev_mask.bits()),
            Some(((unmap as u32) << 31) | u32::try_from(self.number).unwrap()),
            OutputParam::Empty,
        )?;
        Ok(())
    }

    /// Unmap all events from this EQ.
    fn unmap(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        let unmap = true;
        cmd.execute_command(
            Opcode::MapEq,
            None,
            InputParam::Immediate(self.async_ev_mask.bits()),
            Some(((unmap as u32) << 31) | u32::try_from(self.number).unwrap()),
            OutputParam::Empty,
        )?;
        self.async_ev_mask = AsyncEventMask::empty();
        Ok(())
    }

    /// Destroy the event queue.
    pub(super) fn destroy(&mut self, cmd: &mut CommandInterface) -> Result<(), &'static str> {
        if !self.async_ev_mask.is_empty() {
            self.unmap(cmd)?;
        }
        cmd.execute_command(Opcode::Hw2SwEq, None, InputParam::Empty, Some(self.number.try_into().unwrap()), OutputParam::Empty)?;
        // actually free the memory
        self.memory.take().unwrap();
        Ok(())
    }

    /// Ring this event queue by writing the consumer index to the appropriate
    /// doorbell.
    ///
    /// If armed, events will generate interrupts.
    fn ring(&self, arm: bool) {
        // There are four EQE doorbell per page
        let doorbell: &mut DoorbellPage = unsafe { &mut *self.doorbell_page.start_address().as_mut_ptr()};
        doorbell.eqs[self.number % 4].val.set(((*self.consumer_index.lock() & 0xffffff) | (arm as u32) << 31).to_be());
        compiler_fence(Ordering::SeqCst);
    }

    /// Handle events, returning how many were consumed.
    ///
    /// Called from the EQ's interrupt handler.
    pub(super) fn handle_events(&self) -> usize {
        let mut handled = 0;
        while let Some(ref eqe) = self.consume_entry() {
            report_event(eqe);
            handled += 1;
        }

        self.ring(true);

        handled
    }

    /// Consumes one EQE from the event queue
    fn consume_entry(&self) -> Option<EventQueueEntry> {
        let mut index = self.consumer_index.lock();
        let buffer_start_addr: *const EventQueueEntry = self.memory.unwrap().0.page_range().start.start_address().as_ptr();
        // wrap around after num_entries
        let eqe_start = unsafe { buffer_start_addr.add((*index & (self.num_entries - 1)) as usize) };
        let word_count = size_of::<EventQueueEntry>() / size_of::<u32>();
        // check ownership before reading the EQE
        let last_word = unsafe { u32::from_be(eqe_start.cast::<u32>().add(word_count - 1).read_volatile()) };
        let owner = (last_word >> 7) & 1;
        let round = (*index / self.num_entries) & 1;
        if owner == round {
            return None;
        }
        *index += 1;
        compiler_fence(Ordering::SeqCst);

        // TODO: ConnectX-3 is capable of extending the EQE from 32 to 64 bytes
        // with strides of 64B, 128B and 256B. When 64B EQE is used, the
        // first (in the lower addresses) 32 bytes in the 64 byte EQE are
        // reserved and the next 32 bytes contain the legacy EQE information.
        Some(unsafe { eqe_start.read_volatile() })
    }

    /// Get the number of this event queue.
    pub(super) fn number(&self) -> usize {
        self.number
    }
}

/// Report an event the card has posted.
///
/// These are the only notice the card gives that something has gone wrong on its side: a queue
/// pair or completion queue moved to the error state, a port that left the active state, or the
/// card itself failing. They used to be dropped on the floor, which is why failures like the
/// port falling back to `Initializing` showed up with nothing at all in the log to explain them.
///
/// Everything except completions is rare, so it is logged unconditionally; a completion event
/// arrives per armed completion queue and stays at `trace`.
fn report_event(eqe: &EventQueueEntry) {
    let raw_type = eqe.event_type();
    let Some(event_type) = EventType::from_repr(raw_type.into()) else {
        warn!("got an event of unknown type {raw_type:#04x}, subtype {:#04x}", eqe.event_subtype());
        return;
    };
    match event_type {
        EventType::Completion => trace!("completion event for CQ {}", eqe.cq_number()),

        // The card is telling us it is broken. After this it may keep the link up while no
        // longer answering the subnet manager, so nothing else will report it.
        EventType::InternalError | EventType::FatalWarning => {
            error!("the card reported {event_type:?} (subtype {:#04x}, {:08x?})", eqe.event_subtype(), eqe.event_words());
        }
        EventType::CqError => {
            error!("completion queue {} is in error, syndrome {:#04x}", eqe.cq_number(), eqe.cq_error_syndrome());
        }
        EventType::WqCatastrophicError | EventType::WqInvalidRequestError | EventType::WqAccessViolation | EventType::PathMigrationFailed => {
            error!("queue pair {} reported {event_type:?}", eqe.qp_number());
        }

        // A port leaving the active state is the symptom we are chasing: subtype 1 is down,
        // subtype 4 is active again.
        EventType::PortChange => {
            let state = match eqe.event_subtype() {
                1 => "down",
                4 => "active",
                other => {
                    warn!("port {} changed to unknown state {other:#04x}", eqe.port());
                    return;
                }
            };
            warn!("port {} is now {state}", eqe.port());
        }
        EventType::PortManagementChange => {
            warn!("the subnet manager reconfigured port {} (subtype {:#04x})", eqe.port(), eqe.event_subtype());
        }

        // Everything else is informational, but still worth seeing while the data path is
        // being brought up.
        other => debug!("got event {other:?} (subtype {:#04x}, {:08x?})", eqe.event_subtype(), eqe.event_words()),
    }
}

pub struct EventQueueHandler {
    eq: Arc<RwLock<EventQueue>>,
    clr_int: ClrInt,
}

impl EventQueueHandler {
    pub(crate) fn new(eq: Arc<RwLock<EventQueue>>, clr_int: ClrInt) -> EventQueueHandler {
        Self { eq, clr_int }
    }
}
impl InterruptHandler for EventQueueHandler {
    fn trigger(&self) {
        self.clr_int.clear();
        if let Some(eq) = self.eq.try_read() {
            eq.handle_events();
        } else {
            trace!("failed to aquire read lock")
        }
    }
}

impl Drop for EventQueue {
    fn drop(&mut self) {
        if self.memory.is_some() {
            panic!("please destroy instead of dropping")
        }
    }
}

#[bitfield]
struct EventQueueContext {
    #[skip(setters)]
    status: B4,
    #[skip]
    __: B16,
    #[skip(getters)]
    state: B4,
    #[skip]
    __: B60,
    #[skip]
    page_offset: B7,
    #[skip]
    __: u8,
    #[skip(getters)]
    log_eq_size: B5,
    #[skip]
    __: B24,
    eq_period: u16,
    eq_max_count: u16,
    #[skip]
    __: B22,
    #[skip(getters)]
    intr: B10,
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
    __: B72,
    #[skip(setters)]
    consumer_index: B24,
    #[skip]
    __: u8,
    #[skip(setters)]
    producer_index: B24,
    #[skip]
    __: B96,
}

#[bitfield(bytes = 32)]
struct EventQueueEntry {
    #[skip]
    __: u8,
    event_type: u8,
    #[skip]
    __: u8,
    #[skip(setters)]
    event_subtype: u8,
    #[skip(setters)]
    event_data1: B96,
    #[skip]
    event_data2: B96,
    #[skip]
    __: B24,
    owner: bool,
    #[skip]
    __: B7,
}

impl EventQueueEntry {
    /// The first three words of the event body, which is where every event type this driver
    /// cares about puts its payload (`union mlx4_eqe.event` in the reference driver).
    ///
    /// `event_data1` is stored most significant bit first, so the first word is at the top.
    fn event_words(&self) -> [u32; 3] {
        let data = self.event_data1();
        [(data >> 64) as u32, (data >> 32) as u32, data as u32]
    }

    /// The port a port change event refers to.
    fn port(&self) -> u32 {
        self.event_words()[2] >> 28
    }

    /// The queue pair a work queue event refers to.
    fn qp_number(&self) -> u32 {
        self.event_words()[0] & 0xff_ffff
    }

    /// The completion queue a completion queue event refers to.
    fn cq_number(&self) -> u32 {
        self.event_words()[0] & 0xff_ffff
    }

    /// The syndrome of a completion queue error.
    ///
    /// `struct mlx4_eqe.event.cq_err` is `{ u32 cqn; u32 reserved1; u8 reserved2[3]; u8
    /// syndrome; }`: the syndrome byte is the low byte of the third word, not the second (which
    /// is always-zero padding and was being misread as the syndrome before).
    fn cq_error_syndrome(&self) -> u32 {
        self.event_words()[2] & 0xff
    }
}

impl core::fmt::Debug for EventQueueEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EventQueueEntry")
            .field("owner", &self.owner())
            .field("type", &EventType::from_repr(self.event_type().into()))
            .finish_non_exhaustive()
    }
}

#[repr(u64)]
#[derive(Debug, FromRepr)]
enum EventType {
    // completion
    Completion = 0x00,

    // IB affiliated events
    PathMigrationSucceeded = 0x01,
    CommunicationEstablished = 0x02,
    SendQueueDrained = 0x03,
    SrqLastWqe = 0x13,
    SrqLimit = 0x14,

    // QP affiliated errors
    CqError = 0x04,
    WqCatastrophicError = 0x05,
    EecCatastrophicError = 0x06,
    PathMigrationFailed = 0x07,
    WqInvalidRequestError = 0x10,
    WqAccessViolation = 0x11,
    SrqCatastropicError = 0x12,

    // unaffiliated events and errors
    InternalError = 0x08,
    PortChange = 0x09,
    // EqOverflow = 0x0f,
    // EccDetect = 0x0e,
    // VepUpdate = 0x19,
    // OpRequired = 0x1a,
    FatalWarning = 0x1b,
    FlrEvent = 0x1c,
    PortManagementChange = 0x1d,
    RecoverableEvent = 0x3e,
    // None = 0xff,

    // HCA interface
    CommandInterfaceCompletion = 0x0a,
    CommunicationChannelWritten = 0x18,
}

bitflags! {
    #[derive(Debug)]
    pub struct AsyncEventMask: u64 {
        // IB affiliated
        const PATH_MIGRATION_SUCCEEDED = 1 << EventType::PathMigrationSucceeded as u64;
        const COMMUNICATION_ESTABLISHED = 1 << EventType::CommunicationEstablished as u64;
        const SEND_QUEUE_DRAINED = 1 << EventType::SendQueueDrained as u64;
        const SRQ_LAST_WQE = 1 << EventType::SrqLastWqe as u64;
        const SRQ_LIMIT = 1 << EventType::SrqLimit as u64;

        // QP affiliated errors
        const CQ_ERROR = 1 << EventType::CqError as u64;
        const WQ_CATASTROPHIC_ERROR = 1 << EventType::WqCatastrophicError as u64;
        const EEC_CATASTROPHIC_ERROR = 1 << EventType::EecCatastrophicError as u64;
        const PATH_MIGRATION_FAILED = 1 << EventType::PathMigrationFailed as u64;
        const WQ_INVALID_REQUEST_ERROR = 1 << EventType::WqInvalidRequestError as u64;
        const WQ_ACCESS_VIOLATION = 1 << EventType::WqAccessViolation as u64;
        const SRQ_CATASTROPHIC_ERROR = 1 << EventType::SrqCatastropicError as u64;

        // unaffiliated events and errors
        const INTERNAL_ERROR = 1 << EventType::InternalError as u64;
        const PORT_CHANGE = 1 << EventType::PortChange as u64;
        const FATAL_WARNING = 1 << EventType::FatalWarning as u64;
        const FLR_EVENT = 1 << EventType::FlrEvent as u64;
        const PORT_MANAGEMENT_CHANGE = 1 << EventType::PortManagementChange as u64;
        const RECOVERABLE_EVENT = 1 << EventType::RecoverableEvent as u64;

        // HCA interface
        const COMMAND_INTERFACE_COMPLETION = 1 << EventType::CommandInterfaceCompletion as u64;
        const COMMUNICATION_CHANNEL_WRITTEN = 1 << EventType::CommunicationChannelWritten as u64;
    }
}
