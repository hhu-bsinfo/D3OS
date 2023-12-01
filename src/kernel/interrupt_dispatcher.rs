use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;
use crate::kernel;
use crate::kernel::isr::ISR;

#[repr(u8)]
#[derive(PartialEq, PartialOrd, Copy, Clone)]
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
    Mouse = 0x2c,
    Fpu = 0x2d,
    PrimaryAta = 0x2e,
    SecondaryAta = 0x2f,
    // Possibly some other interrupts supported by IO APICs

    SystemCall = 0x86,

    // Local APIC interrupts (247 - 254)
    Cmci = 0xf8,
    ApicTimer = 0xf9,
    Thermal = 0xfa,
    Performance = 0xfb,
    Lint0 = 0xfc,
    Lint1 = 0xfd,
    ApicError = 0xfe,

    Spurious = 0xff
}

impl TryFrom<u8> for InterruptVector {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            value if value == InterruptVector::Pit as u8 => Ok(InterruptVector::Pit),
            value if value == InterruptVector::Keyboard as u8 => Ok(InterruptVector::Keyboard),
            value if value == InterruptVector::Cascade as u8 => Ok(InterruptVector::Cascade),
            value if value == InterruptVector::Com2 as u8 => Ok(InterruptVector::Com2),
            value if value == InterruptVector::Com1 as u8 => Ok(InterruptVector::Com1),
            value if value == InterruptVector::Lpt2 as u8 => Ok(InterruptVector::Lpt2),
            value if value == InterruptVector::Floppy as u8 => Ok(InterruptVector::Floppy),
            value if value == InterruptVector::Lpt1 as u8 => Ok(InterruptVector::Lpt1),
            value if value == InterruptVector::Mouse as u8 => Ok(InterruptVector::Mouse),
            value if value == InterruptVector::Fpu as u8 => Ok(InterruptVector::Fpu),
            value if value == InterruptVector::PrimaryAta as u8 => Ok(InterruptVector::PrimaryAta),
            value if value == InterruptVector::SecondaryAta as u8 => Ok(InterruptVector::SecondaryAta),
            _ => Err(())
        }
    }
}

const MAX_VECTORS: usize = 256;

pub struct InterruptDispatcher {
    int_vectors: Vec<Mutex<Vec<Box<dyn ISR>>>>
}

unsafe impl Send for InterruptDispatcher {}
unsafe impl Sync for InterruptDispatcher {}

impl InterruptDispatcher {
    pub const fn new() -> Self {
        Self { int_vectors: Vec::new() }
    }

    pub fn init(&mut self) {
        for _ in 0..MAX_VECTORS {
            self.int_vectors.push(Mutex::new(Vec::new()));
        }
    }

    pub fn assign(&mut self, vector: InterruptVector, isr: Box<dyn ISR>) {
        match self.int_vectors.get(vector as usize) {
            Some(vec) => vec.lock().push(isr),
            None => panic!("Assigning ISR to illegal vector number {}!", vector as u8)
        }
    }

    pub fn dispatch(&mut self, int_number: u32) {
        if let Some(isr_vec_mutex) = self.int_vectors.get(int_number as usize).as_mut() {
            let mut isr_vec = isr_vec_mutex.try_lock();
            while isr_vec_mutex.is_locked() {
                // We have to force unlock inside the interrupt handler, or else the system will hang forever.
                // While this might be unsafe, it is extremely unlikely that we destroy something here, since we only need read access to the vectors.
                // The only scenario, in which something might break, is when two or more drivers are trying to assign an ISR to the same vector,
                // while an interrupt for that vector occurs.
                unsafe {
                    isr_vec_mutex.force_unlock();
                    isr_vec = isr_vec_mutex.try_lock();
                }
            }

            for isr in isr_vec.unwrap().iter() {
                isr.trigger();
            }
        }

        kernel::get_interrupt_service().end_of_interrupt();
    }
}