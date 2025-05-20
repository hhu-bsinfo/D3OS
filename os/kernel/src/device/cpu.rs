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

        info!("Cpu: Physical address bits {}, Linear address bits {}, supports_1gib_pages = {}", physical_bits, virtual_bits, has_1gib_pages);
    
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


}
