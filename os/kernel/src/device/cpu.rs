/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: cpu                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Retrieve and store cpu features using cpuid.                            ║
   ║                                                                         ║
   ║ Public functions                                                        ║
   ║   - highest_virtual_address       Return the highest virtual address    ║
   ║   - highest_physical_address      Return the highest physical address   ║
   ║   - disable_int_nested            Disable interrupts                    ║
   ║   - enable_int_nested             Enable interrupts                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 01.09.2025, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use log::info;
use raw_cpuid::CpuId;
use core::arch::asm;
use core::arch::x86_64::{_xrstor, _xsave};
use bitflags::bitflags;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
use x86_64::registers::xcontrol::{XCr0, XCr0Flags};
use x86_64::structures::paging::frame::PhysFrameRange;
use crate::cpu;
use crate::memory::{alloc_frames, free_frames, PAGE_SIZE};

bitflags! {
    /// Flags for CPU extension states that can be saved and restored using the XSAVE/XRSTOR instructions.
    pub struct XSaveComponents: u64 {
        const X87_FPU = 1 << 0;
        const SSE = 1 << 1;
        const AVX = 1 << 2;
        const MPX_BNDREGS = 1 << 3;
        const MPX_BNDCSR = 1 << 4;
        const AVX512_OPMASK = 1 << 5;
        const AVX512_ZMM_HI256 = 1 << 6;
        const AVX512_HI16_ZMM = 1 << 7;
        const PT = 1 << 8;
        const PKRU = 1 << 9;
        const PASID = 1 << 10;
        const CET_U = 1 << 1;
        const CET_S = 1 << 12;
        const HDC = 1 << 13;
        const UINTR = 1 << 14;
        const LBR = 1 << 15;
        const HWP = 1 << 16;
        const AMX_TILECFG = 1 << 17;
        const AMX_TILEDATA = 1 << 18;
    }
}

/// A struct that can be used to save/restore CPU extension states using the XSAVE/XRSTOR instructions.
/// It just allocates a single page frame and uses it as a data area for XSAVE/XRSTOR.
/// CAUTION: Storing the state of all CPU extensions might write of bounds of the page frame.
///          But a page frame is definitely large enough to hold X87, SSE, and AVX states at once.
pub struct XSaveState {
    xsave_area: PhysFrameRange
}

impl XSaveState {
    /// Create a new XSaveState instance by cloning the initial state.
    /// Just using a zeroed page frame would be dangerous, as restoring from it would fail, since it does not hold a valid XSAVE structure.
    /// Thus, we just copy the page frame stored in `cpu::initial_xsave_state`, which holds the xsave state that is created during boot.
    pub fn new() -> Self {
        cpu().initial_xsave_state.clone()
    }
}

impl Clone for XSaveState {
    fn clone(&self) -> Self {
        let initial_xsave_frames = cpu().initial_xsave_state.xsave_area;
        let xsave_frames = alloc_frames(1);

        unsafe {
            let source_ptr = initial_xsave_frames.start.start_address().as_u64() as *mut u8;
            let target_ptr = xsave_frames.start.start_address().as_u64() as *mut u8;
            target_ptr.copy_from_nonoverlapping(source_ptr, xsave_frames.len() as usize * PAGE_SIZE);
        }

        Self { xsave_area: xsave_frames }
    }
}

impl Drop for XSaveState {
    fn drop(&mut self) {
        free_frames(self.xsave_area);
    }
}

pub struct Cpu {
    physical_address_bits: u8,
    linear_address_bits: u8,
    supports_1gib_pages: bool,
    initial_xsave_state: XSaveState
}

impl Cpu {
    pub fn new() -> Self {
        let physical_bits;
        let virtual_bits;
        let mut has_1gib_pages: bool = false;

        let cpuid = CpuId::new();

        match cpuid.get_processor_capacity_feature_info() {
            None => panic!("CPU: Failed to read CPU ID features!"),
            Some(extended_feature_info) => {
                physical_bits = extended_feature_info.physical_address_bits();
                virtual_bits = extended_feature_info.linear_address_bits();
            }
        }

        match cpuid.get_extended_processor_and_feature_identifiers() {
            None => {
                panic!("CPU: Failed to read extended processor features (CPUID 0x80000001)");
            }
            Some(features) => {
                if features.has_1gib_pages() {
                    has_1gib_pages = true;
                }
            }
        }

        info!("Cpu: Physical address bits {physical_bits}, Linear address bits {virtual_bits}, supports_1gib_pages = {has_1gib_pages}");

        let initial_xsave_state = unsafe {
            // Allocate and clear a frame for the xsave area
            let frames = alloc_frames(1);
            let ptr = frames.start.start_address().as_u64() as *mut u8;
            ptr.write_bytes(0, PAGE_SIZE);

            // Store the xsave area in an `XSaveState` struct.
            // We assume that all extended CPU registers are in their initial state when this is called.
            let mut xsave_state = XSaveState { xsave_area: frames };
            xsave(&mut xsave_state, XSaveComponents::X87_FPU | XSaveComponents::SSE | XSaveComponents::AVX);
            
            xsave_state
        };

        Cpu {
            physical_address_bits: physical_bits,
            linear_address_bits: virtual_bits,
            supports_1gib_pages: has_1gib_pages,
            initial_xsave_state
        }
    }

    pub fn physical_address_bits(&self) -> u8 {
        self.physical_address_bits
    }

    pub fn linear_address_bits(&self) -> u8 {
        self.linear_address_bits
    }

    pub fn supports_1gib_pages(&self) -> bool {
        self.supports_1gib_pages
    }

    /// Return the highest virtual address in canonical form
    pub fn highest_virtual_address(&self) -> u64 {
        let virtual_bits = self.linear_address_bits();
        (1u64 << (virtual_bits - 1)) - 1
    }

    /// Return the highest physical address
    pub fn highest_physical_address(&self) -> u64 {
       // let physical_bits = self.physical_address_bits();
        (1u64 << self.physical_address_bits) - 1
    }
}

/// Disable interrupts and return whether they were previously enabled.
/// This function is used together with 'enable_int_nested' to prevent
/// interrupts from being accidentally enabled.
pub fn disable_int_nested() -> bool {
    let was_enabled = is_int_enabled();
    disable_int();
    was_enabled
}

/// Enable interrupts if 'was_enabled' is true.
/// This function is used together with 'disable_int_nested'.
pub fn enable_int_nested(was_enabled: bool) {
    if was_enabled == true {
        enable_int();
    }
}

fn enable_int() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

fn disable_int() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

fn is_int_enabled() -> bool {
    let rflags: u64;

    unsafe { asm!("pushf; pop {}", lateout(reg) rflags, options(nomem, nostack, preserves_flags)) };
    if (rflags & (1u64 << 9)) != 0 {
        return true;
    }
    false
}

pub fn pause() {
    unsafe {
        asm!("pause", options(nomem, nostack));
    }
}

pub fn enable_simd() {
    unsafe {
        // Enable SSE, AVX and the XSAVE feature
        Cr4::update(|flags| flags.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXSAVE | Cr4Flags::OSXMMEXCPT_ENABLE));
        Cr0::update(|flags| {
            flags.remove(Cr0Flags::EMULATE_COPROCESSOR);
            flags.insert(Cr0Flags::MONITOR_COPROCESSOR);
        });
        XCr0::update(|flags| flags.insert(XCr0Flags::X87 | XCr0Flags::SSE | XCr0Flags::AVX));
    }
}

pub fn enable_fsgsbase() {
    unsafe {
        Cr4::update(|flags| flags.insert(Cr4Flags::FSGSBASE))
    }
}

/// Store the specified CPU extension states in the given XSaveState struct.
/// CAUTION: Storing the state of all CPU extensions might write of bounds of the XSaveState struct.
///          But a page frame is definitely large enough to hold X87, SSE, and AVX states at once.
pub fn xsave(xsave_state: &XSaveState, components: XSaveComponents) {
    unsafe {
        _xsave(xsave_state.xsave_area.start.start_address().as_u64() as *mut u8, components.bits())
    }
}

/// Restore the specified CPU extension states from the given XSaveState struct.
pub fn xrstor(xsave_state: &XSaveState, components: XSaveComponents) {
    unsafe {
        _xrstor(xsave_state.xsave_area.start.start_address().as_u64() as *mut u8, components.bits())
    }
}