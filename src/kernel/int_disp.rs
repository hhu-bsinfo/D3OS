use alloc::boxed::Box;
use alloc::collections::LinkedList;
use alloc::vec::Vec;
use spin::Mutex;
use crate::device::apic;
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

struct IntVectors {
    map: Vec<LinkedList<Box<dyn ISR>>>,
}

unsafe impl Send for IntVectors {}
unsafe impl Sync for IntVectors {}

static INT_VECTORS: Mutex<IntVectors> = Mutex::new(IntVectors { map: Vec::new() });

#[no_mangle]
pub extern "C" fn int_disp(int_number: u32) {
    if let Ok(vector) = InterruptVector::try_from(int_number as u8) {
        // Force unlock needed to avoid possible deadlock on INT_VECTORS.
        // This can for example happen, if an interrupt occurs directly after registering its ISR, but before the INT_VECTORS mutex has been released
        unsafe { INT_VECTORS.force_unlock(); }
        let vectors = INT_VECTORS.lock();
        let isr_list = vectors.map.get(vector as usize);
        isr_list.unwrap().iter().for_each(|isr| {
            isr.trigger();
        });

        // Force unlock needed to avoid possible deadlock on APIC.
        // This can for example happen, if an interrupt occurs directly after enabling it in the IO APIC, but before the APIC mutex has been released
        unsafe { apic::get_apic().force_unlock(); }
        apic::get_apic().lock().send_eoi(vector);
    }
}

pub fn init() {
    let mut vectors = INT_VECTORS.lock();

    for _ in 0..MAX_VECTORS {
        vectors.map.push(LinkedList::new());
    }
}

pub fn assign(vector: InterruptVector, isr: Box<dyn ISR>) {
    let mut vectors = INT_VECTORS.lock();
    vectors.map[vector as usize].push_back(isr);
}