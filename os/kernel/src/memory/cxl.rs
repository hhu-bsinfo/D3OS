use alloc::vec::Vec;
use acpi::{AcpiTable};
use acpi::sdt::{SdtHeader, Signature};
use log::info;
use uefi::table::boot::PAGE_SIZE;
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::memory::nvmem::Nfit;
use crate::{acpi_tables, pci_bus, process_manager};
use crate::memory::MemorySpace;

pub fn print_bus_devices(){
    pci_bus().dump_devices();
}

pub fn print_bus_devices_status(){
    pci_bus().dump_devices_status_registers();
}

pub fn print_bus_devices_command(){
    pci_bus().dump_devices_command_registers();
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CEDT {
    header: SdtHeader,
    cedt_structures: u32,   //in der Dokumentation steht, dass es variieren kann, je nachdem, wie viele Strukturen da sein
                            //sieht vom header aehnlich aus, wie bei nvmm, deswegen habe ich mal u32 genommen
}

#[allow(dead_code)]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq)]
enum CEDTStructureType {
    CXLHostBridgeStructure = 0,
    CXLFixedMemoryWindowStructure = 1,
    CXLXORInterleaveMathStructure = 2,
    RCECDownstreamPortAssociationStructure = 3,
    CXLSystemDescriptionStructure = 4,
}


#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CXLHostBridgeStructure{
    typ: u8,
    reserved_1: u8,
    record_length: u16,
    uid: u32,
    cxl_version: u32,
    reserved_2: u32,
    base: u64,
    length: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CXLFixedMemoryWindowStructure{
    typ: u8,
    reserved_1: u8,
    record_length: u16,
    reserved_2: u32,
    base_hpa: u64,
    window_size: u64,
    encoded_nr_of_interleave_ways: u8,
    interleave_arithmetic: u8,
    reserved_3: u16,
    host_bridge_interleave_granularity: u64,
    window_restrictions: u16,
    qtg_id: u16,
    interleave_target_list: u64, //hier ist die groesse 4* Anzahl encodet interleave ways
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CXLXORInterleaveMathStructure{
    typ: u8,
    reserved_1: u8,
    record_length: u16,
    reserved_2: u16,
    nr_of_bitmap_entries: u8,
    xormap_list: u128, // hier muss 8*Anzahl vor nr_of_bitmap_entries
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RCECDownstreamPortAssociationStructure{
    typ: u8,
    reserved_1: u8,
    record_length: u16,
    rcec_segment_nr: u16,
    rcec_bdf: u16,
    protocol_type: u16,
    base_addr: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CXLSystemDescriptionStructure{
    typ: u8,
    reserved_1: u8,
    record_length: u16,
    system_capabilities: u16,
    reserved_2: u16,
}

unsafe impl AcpiTable for CEDT {
    const SIGNATURE: Signature = Signature::CEDT;

    fn header(&self) -> &SdtHeader {
        &self.header
    }
}




pub fn init() {
    if let Ok(nfit) = acpi_tables().lock().find_table::<CEDT>() {
        info!("Found CEDT table");
    }
}