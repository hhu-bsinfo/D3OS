/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: nvmem                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Support of NVRAM.                                                       ║
   ║   - init   find and map NVRAM in kernel space                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 29.6.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::memory::vma::VmaType;
use crate::memory::PAGE_SIZE;
use crate::{acpi_tables, process_manager};
use acpi::AcpiTable;
use acpi::sdt::{SdtHeader, Signature};
use alloc::vec::Vec;
use bitflags::bitflags;
use core::cmp::PartialEq;
use core::ptr;
use log::info;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::{PageTableFlags, PhysFrame};

#[allow(dead_code)]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum NfitStructureType {
    SystemPhysicalAddressRange = 0,
    NvdimmRegionMappingStructure = 1,
    Interleave = 2,
    SmbiosManagementInformation = 3,
    NvdimmControlRegion = 4,
    NvdimmBlockDataWindowRegion = 5,
    FlushHintAddress = 6,
    PlatformCapabilities = 7,
}

bitflags! {
    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct AddressRangeMemoryMappingAttribute: u32 {
        const UC = 0x00000001;
        const WC = 0x00000002;
        const WT = 0x00000004;
        const WB = 0x00000008;
        const UCE = 0x00000010;
        const WP = 0x00001000;
        const RP = 0x00002000;
        const XP = 0x00004000;
        const NV = 0x00008000;
        const MORE_RELIABLE = 0x00010000;
        const RO = 0x00020000;
        const SP = 0x00040000;
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Nfit {
    header: SdtHeader,
    reserved: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct NfitStructureHeader {
    typ: NfitStructureType,
    length: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct SystemPhysicalAddressRange {
    header: NfitStructureHeader,
    spa_range_structure_index: u16,
    flags: u16,
    reserved: u32,
    proximity_domain: u32,
    address_range_type_guid: u128,
    base: u64,
    length: u64,
    mapping_attributes: AddressRangeMemoryMappingAttribute,
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct NvdimmRegionMappingStructure {
    header: NfitStructureHeader,
    nfit_device_handle: u32,
    physical_id: u16,
    region_id: u16,
    spa_range_structure_index: u16,
    control_region_structure_index: u16,
    region_size: u64,
    region_offset: u64,
    phys_addr_region_base: u64,
    interleave_structure_index: u16,
    interleave_ways: u16,
    state_flags: u16,
    reserved: [u8; 2],
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct NvdimmControlRegionStructure {
    header: NfitStructureHeader,
    control_region_structure_index: u16,
    vendor_id: u16,
    device_id: u16,
    revision_id: u16,
    subsystem_vendor_id: u16,
    subsystem_device_id: u16,
    subsystem_revision_id: u16,
    valid_fields: u8,
    manufacturing_location: u8,
    manufacturing_date: u16,
    reserved1: [u8; 2],
    serial_number: u32,
    region_format_interface_code: u16,
    window_count: u16,
    window_size: u64,
    command_register_offset: u64,
    command_register_size: u64,
    status_register_offset: u64,
    status_register_size: u64,
    control_region_size: u64,
    control_region_flags: u16,
    reserved2: [u8; 6],
}

#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FlushHintAddressStructure {
    header: NfitStructureHeader,
    device_handle: u32,
    hint_count: u16,
}

unsafe impl AcpiTable for Nfit {
    const SIGNATURE: Signature = Signature::NFIT;

    fn header(&self) -> &SdtHeader {
        &self.header
    }
}

impl Nfit {
    pub fn get_structures(&self) -> Vec<&NfitStructureHeader> {
        let mut tables = Vec::<&NfitStructureHeader>::new();

        let mut remaining = self.header.length as usize - size_of::<Nfit>();
        let mut structure_ptr = unsafe { ptr::from_ref(self).add(1) } as *const NfitStructureHeader;

        while remaining > 0 {
            unsafe {
                let structure = *structure_ptr;
                tables.push(structure_ptr.as_ref().expect("Invalid NFIT structure"));

                structure_ptr = (structure_ptr as *const u8).add(structure.length as usize) as *const NfitStructureHeader;
                remaining -= structure.length as usize;
            }
        }

        tables
    }

    pub fn get_phys_addr_ranges(&self) -> Vec<&SystemPhysicalAddressRange> {
        let mut ranges = Vec::<&SystemPhysicalAddressRange>::new();

        self.get_structures().iter().for_each(|structure| {
            let structure_type = unsafe { ptr::from_ref(structure).read_unaligned().typ };
            if structure_type == NfitStructureType::SystemPhysicalAddressRange {
                ranges.push(structure.as_structure::<SystemPhysicalAddressRange>());
            }
        });

        ranges
    }
}

impl NfitStructureHeader {
    pub fn as_structure<T>(&self) -> &T {
        unsafe { ptr::from_ref(self).cast::<T>().as_ref().expect("Invalid NFIT structure") }
    }
}

impl SystemPhysicalAddressRange {
    pub fn as_phys_frame_range(&self) -> PhysFrameRange {
        let start = PhysFrame::from_start_address(PhysAddr::new(self.base)).expect("Invalid start address");

        PhysFrameRange {
            start,
            end: start + (self.length / PAGE_SIZE as u64),
        }
    }
}

impl FlushHintAddressStructure {
    pub fn get_flush_hint_addresses(&self) -> Vec<u64> {
        let mut hints = Vec::new();
        for i in 0..self.hint_count as usize {
            unsafe {
                let hint_ptr = (ptr::from_ref(self).add(1) as *const u64).add(i);
                let hint = hint_ptr.read_unaligned();
                hints.push(hint);
            }
        }

        hints
    }
}

pub fn init() {
    info!("Found NFIT table");

    let process = process_manager().read().kernel_process().expect("Failed to get kernel process");
    if let Ok(nfit) = acpi_tables().lock().find_table::<Nfit>() {
        // Search NFIT table for non-volatile memory ranges
        for spa in nfit.get_phys_addr_ranges() {

            // Copy values to avoid unaligned access of packed struct fields
            let address = spa.base;
            let length = spa.length;
            info!("Found non-volatile memory (Address: [0x{:x}], Length: [{} MiB])", address, length / 1024 / 1024);
            
            process.virtual_address_space.kernel_map_devm_identity(
                address,
                address + length,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                VmaType::DeviceMemory,
                "nvram",
            );
        }
    }
}
