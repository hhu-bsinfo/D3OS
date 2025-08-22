/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: cpu                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Retrieve and store cpu features using cpuid.                            ║
   ║                                                                         ║
   ║ Public functions                                                        ║
   ║   - highest_virtual_address       Return the highest virtual address    ║
   ║   - highest_physical_address      Return the highest physical address   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, HHU                                         ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use log::info;
use raw_cpuid::CpuId;

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

    /// Return the highest virtual address in canonical form
    pub fn highest_virtual_address(&self) -> u64 {
        let virtual_bits = self.linear_address_bits();
        (1u64 << (virtual_bits - 1)) - 1
    }

    /// Return the highest physical address
    pub fn highest_physical_address(&self) -> u64 {
        let physical_bits = self.physical_address_bits();
        (1u64 << self.physical_address_bits) - 1
    }

}
