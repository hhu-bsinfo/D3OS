use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::vma::VmaType;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use spin::Mutex;
use x86_64::registers::control::Cr2;
use x86_64::set_general_handler;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use crate::{apic, idt, interrupt_dispatcher, scheduler};
use crate::memory::MemorySpace;

#[repr(u8)]
#[derive(PartialEq, PartialOrd, Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum InterruptVector {
    // Hardware exceptions
    DivisionByZero = 0,
    Debug = 1,
    NonMaskableInterrupt = 2,
    Breakpoint = 3,
    Overflow = 4,
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    CoprocessorSegmentOverrun = 9,
    InvalidTaskStateSegment = 10,
    SegmentNotPresent = 11,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    X87FloatingPointException = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SimdFloatingPointException = 19,
    VirtualizationException = 20,
    ControlProtectionException = 21,
    HypervisorInjectionException = 28,
    VmmCommunicationException = 29,
    SecurityException = 30,

    // PC/AT compatible interrupts
    Pit = 0x20,
    Keyboard = 0x21,
    Cascade = 0x22,
    Com2 = 0x23,
    Com1 = 0x24,
    Lpt2 = 0x25,
    Floppy = 0x26,
    Lpt1 = 0x27,
    Rtc = 0x28,
    Free1 = 0x29,
    Free2 = 0x2a,
    Free3 = 0x2b,
    Mouse = 0x2c,
    Fpu = 0x2d,
    PrimaryAta = 0x2e,
    SecondaryAta = 0x2f,
    // Possibly some other interrupts supported by IO APICs

    // Local APIC interrupts (247 - 254)
    Cmci = 0xf8,
    ApicTimer = 0xf9,
    Thermal = 0xfa,
    Performance = 0xfb,
    Lint0 = 0xfc,
    Lint1 = 0xfd,
    ApicError = 0xfe,

    Spurious = 0xff,
}

impl TryFrom<u8> for InterruptVector {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            value if value == InterruptVector::DivisionByZero as u8 => {
                Ok(InterruptVector::DivisionByZero)
            }
            value if value == InterruptVector::Debug as u8 => Ok(InterruptVector::Debug),
            value if value == InterruptVector::NonMaskableInterrupt as u8 => {
                Ok(InterruptVector::NonMaskableInterrupt)
            }
            value if value == InterruptVector::Breakpoint as u8 => Ok(InterruptVector::Breakpoint),
            value if value == InterruptVector::Overflow as u8 => Ok(InterruptVector::Overflow),
            value if value == InterruptVector::BoundRangeExceeded as u8 => {
                Ok(InterruptVector::BoundRangeExceeded)
            }
            value if value == InterruptVector::InvalidOpcode as u8 => {
                Ok(InterruptVector::InvalidOpcode)
            }
            value if value == InterruptVector::DeviceNotAvailable as u8 => {
                Ok(InterruptVector::DeviceNotAvailable)
            }
            value if value == InterruptVector::DoubleFault as u8 => {
                Ok(InterruptVector::DoubleFault)
            }
            value if value == InterruptVector::CoprocessorSegmentOverrun as u8 => {
                Ok(InterruptVector::CoprocessorSegmentOverrun)
            }
            value if value == InterruptVector::InvalidTaskStateSegment as u8 => {
                Ok(InterruptVector::InvalidTaskStateSegment)
            }
            value if value == InterruptVector::SegmentNotPresent as u8 => {
                Ok(InterruptVector::SegmentNotPresent)
            }
            value if value == InterruptVector::StackSegmentFault as u8 => {
                Ok(InterruptVector::StackSegmentFault)
            }
            value if value == InterruptVector::GeneralProtectionFault as u8 => {
                Ok(InterruptVector::GeneralProtectionFault)
            }
            value if value == InterruptVector::PageFault as u8 => Ok(InterruptVector::PageFault),
            value if value == InterruptVector::X87FloatingPointException as u8 => {
                Ok(InterruptVector::X87FloatingPointException)
            }
            value if value == InterruptVector::AlignmentCheck as u8 => {
                Ok(InterruptVector::AlignmentCheck)
            }
            value if value == InterruptVector::MachineCheck as u8 => {
                Ok(InterruptVector::MachineCheck)
            }
            value if value == InterruptVector::SimdFloatingPointException as u8 => {
                Ok(InterruptVector::SimdFloatingPointException)
            }
            value if value == InterruptVector::VirtualizationException as u8 => {
                Ok(InterruptVector::VirtualizationException)
            }
            value if value == InterruptVector::ControlProtectionException as u8 => {
                Ok(InterruptVector::ControlProtectionException)
            }
            value if value == InterruptVector::HypervisorInjectionException as u8 => {
                Ok(InterruptVector::HypervisorInjectionException)
            }
            value if value == InterruptVector::VmmCommunicationException as u8 => {
                Ok(InterruptVector::VmmCommunicationException)
            }
            value if value == InterruptVector::SecurityException as u8 => {
                Ok(InterruptVector::SecurityException)
            }

            value if value == InterruptVector::Pit as u8 => Ok(InterruptVector::Pit),
            value if value == InterruptVector::Keyboard as u8 => Ok(InterruptVector::Keyboard),
            value if value == InterruptVector::Cascade as u8 => Ok(InterruptVector::Cascade),
            value if value == InterruptVector::Com2 as u8 => Ok(InterruptVector::Com2),
            value if value == InterruptVector::Com1 as u8 => Ok(InterruptVector::Com1),
            value if value == InterruptVector::Lpt2 as u8 => Ok(InterruptVector::Lpt2),
            value if value == InterruptVector::Floppy as u8 => Ok(InterruptVector::Floppy),
            value if value == InterruptVector::Lpt1 as u8 => Ok(InterruptVector::Lpt1),
            value if value == InterruptVector::Rtc as u8 => Ok(InterruptVector::Rtc),
            value if value == InterruptVector::Free1 as u8 => Ok(InterruptVector::Free1),
            value if value == InterruptVector::Free2 as u8 => Ok(InterruptVector::Free2),
            value if value == InterruptVector::Free3 as u8 => Ok(InterruptVector::Free3),
            value if value == InterruptVector::Mouse as u8 => Ok(InterruptVector::Mouse),
            value if value == InterruptVector::Fpu as u8 => Ok(InterruptVector::Fpu),
            value if value == InterruptVector::PrimaryAta as u8 => Ok(InterruptVector::PrimaryAta),
            value if value == InterruptVector::SecondaryAta as u8 => {
                Ok(InterruptVector::SecondaryAta)
            }

            value if value == InterruptVector::Cmci as u8 => Ok(InterruptVector::Cmci),
            value if value == InterruptVector::ApicTimer as u8 => Ok(InterruptVector::ApicTimer),
            value if value == InterruptVector::Thermal as u8 => Ok(InterruptVector::Thermal),
            value if value == InterruptVector::Performance as u8 => {
                Ok(InterruptVector::Performance)
            }
            value if value == InterruptVector::Lint0 as u8 => Ok(InterruptVector::Lint0),
            value if value == InterruptVector::Lint1 as u8 => Ok(InterruptVector::Lint1),
            value if value == InterruptVector::ApicError as u8 => Ok(InterruptVector::ApicError),
            value if value == InterruptVector::Spurious as u8 => Ok(InterruptVector::Spurious),
            _ => Err(()),
        }
    }
}

const MAX_VECTORS: usize = 256;

pub struct InterruptDispatcher {
    int_vectors: Vec<Mutex<Vec<Box<dyn InterruptHandler>>>>,
}

unsafe impl Send for InterruptDispatcher {}
unsafe impl Sync for InterruptDispatcher {}

pub fn setup_idt() {
    let mut idt = idt().lock();

    set_general_handler!(&mut idt, handle_exception, 0..31);
    set_general_handler!(&mut idt, handle_interrupt, 32..255);
    set_general_handler!(&mut idt, handle_page_fault, 14);

    unsafe {
        // We need to obtain a static reference to the IDT for the following operation.
        // We know, that it has a static lifetime, since it is are declared as a static variable in 'kernel/mod.rs'.
        // However, since it is hidden behind a Mutex, the borrow checker does not see it with a static lifetime.
        let idt_ref = ptr::from_ref(idt.deref()).as_ref().unwrap();
        idt_ref.load();
    }
}

fn handle_exception(frame: InterruptStackFrame, index: u8, error: Option<u64>) {
    panic!("CPU Exception: [{} - {:?}]\nError code: [{:?}]\n{:?}", index, InterruptVector::try_from(index).unwrap(), error, frame);
}

fn handle_page_fault(frame: InterruptStackFrame, _index: u8, error: Option<u64>) {
    let mut fault_handled = false;
    let fault_addr = Cr2::read().expect("Invalid address in CR2 during page fault");
    let thread = scheduler().current_thread();

    if !thread.is_kernel_thread() {
        // Check if page fault occurred inside a user stack
        let fault_handled = thread.process().virtual_address_space
            .iter_vmas()
            .filter(|vma| vma.typ == VmaType::UserStack)
            .find(|stack| stack.start() <= fault_addr && fault_addr < stack.end())
            .and_then(|stack| {
                // If we found a user stack, we can map the page
                let fault_page = Page::containing_address(fault_addr);
                thread.process().virtual_address_space.map_partial_vma(&stack, PageRange { start: fault_page, end: fault_page + 1, }, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);

                Some(())
            })
            // Check if page fault occurred inside the allocated, but not yet mapped heap.
            .or_else(|| {
                thread.process().virtual_address_space
                    .iter_vmas()
                    .filter(|vma| vma.typ == VmaType::Heap)
                    .find(|heap| heap.start() <= fault_addr && fault_addr < heap.end())
                    .and_then(|heap| {
                        thread.process().grow_heap(&heap, fault_addr);
                        Some(())
                    })
            });

        if fault_handled.is_some() {
            return; // Page fault was handled by mapping the user stack page
        }
    }

    panic!("Page Fault!\nError code: [{:?}]\nAddress: [0x{:0>16x}]\n{:?}", error, fault_addr, frame);
}

fn handle_interrupt(_frame: InterruptStackFrame, index: u8, _error: Option<u64>) {
    interrupt_dispatcher().dispatch(index);
}

impl InterruptDispatcher {
    pub fn new() -> Self {
        let mut int_vectors = Vec::<Mutex<Vec<Box<dyn InterruptHandler>>>>::new();
        for _ in 0..MAX_VECTORS {
            int_vectors.push(Mutex::new(Vec::new()));
        }

        Self { int_vectors }
    }

    pub fn assign(&self, vector: InterruptVector, handler: Box<dyn InterruptHandler>) {
        match self.int_vectors.get(vector as usize) {
            Some(vec) => vec.lock().push(handler),
            None => panic!("Assigning interrupt handler to illegal vector number {}!", vector as u8)
        }
    }

    pub fn dispatch(&self, interrupt: u8) {
        let handler_vec_mutex = self.int_vectors.get(interrupt as usize).unwrap_or_else(|| panic!("Interrupt Dispatcher: No handler vec assigned for interrupt [{}]!", interrupt));
        let mut handler_vec = handler_vec_mutex.try_lock();
        while handler_vec.is_none() {
            // We have to force unlock inside the interrupt handler, or else the system will hang forever.
            // While this might be unsafe, it is extremely unlikely that we destroy something here, since we only need read access to the vectors.
            // The only scenario, in which something might break, is when two or more drivers are trying to assign an interrupt handler to the same vector,
            // while an interrupt for that vector occurs.
            unsafe { handler_vec_mutex.force_unlock(); }
            handler_vec = handler_vec_mutex.try_lock();
        }

        if handler_vec.iter().is_empty() {
            panic!("Interrupt Dispatcher: No handler registered for interrupt [{}]!", interrupt);
        }

        for handler in handler_vec.unwrap().iter_mut() {
            handler.trigger();
        }

        apic().end_of_interrupt();
    }
}
