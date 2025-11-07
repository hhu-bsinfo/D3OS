/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: cpu                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Retrieve and store cpu features using cpuid.                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, HHU                                         ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use log::info;
use raw_cpuid::CpuId;
use core::arch::asm;

pub struct Cpu {
    physical_address_bits: u8,
    linear_address_bits: u8,
    supports_1gib_pages: bool,
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
    
        Cpu {
            physical_address_bits: physical_bits,
            linear_address_bits: virtual_bits,
            supports_1gib_pages: has_1gib_pages,
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

    #[inline(always)]
    pub fn rdtsc(&self) -> u64 {
        let lo: u32;
        let hi: u32;
        unsafe {
            asm!(
                "rdtsc",
                out("eax") lo,
                out("edx") hi,
                options(nomem, nostack, preserves_flags)
            );
        }
        ((hi as u64) << 32) | (lo as u64)
    }

    #[inline(always)]
    pub fn cpuid(&self, eax: u32, ecx: u32) -> (u32, u32, u32) {
        let mut eax_out: u32;
        let mut ecx_out: u32;
        let mut edx_out: u32;

        unsafe {
            asm!(
                "cpuid",
                inout("eax") eax => eax_out,
                inout("ecx") ecx => ecx_out,
                lateout("edx") edx_out,
                // EBX is not captured
                options(nostack, nomem),
            );
        }

        (eax_out, ecx_out, edx_out)
    }

    #[inline(always)]
    pub fn has_tsc(&self) -> bool {
        let (_eax, _ecx, edx) = self.cpuid(1, 0);
        (edx & (1 << 4)) != 0
    }

    #[inline(always)]
    pub fn enable_int () {
        unsafe { asm!("sti"); }
    }

    #[inline(always)]
    pub fn disable_int () {
        unsafe { asm!("cli"); }
    }

    #[inline(always)]
    pub fn local_irq_save(&self) -> u64 {
        let flags: u64;
        unsafe {
            asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) flags,
                options(nomem, preserves_flags)
            );
        }
        flags
    }

    #[inline(always)]
    pub fn local_irq_restore(&self, flags: u64) {
        unsafe {
            asm!(
                "push {}",
                "popfq",
                in(reg) flags,
                options(nomem, preserves_flags)
            );
        }
    }
}
