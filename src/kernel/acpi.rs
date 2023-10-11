use core::ptr::NonNull;
use acpi::{AcpiTables, PhysicalMapping};

static mut TABLES: Option<AcpiTables<AcpiHandler>> = None;

pub fn get_tables() -> &'static AcpiTables<AcpiHandler> {
    unsafe { return TABLES.as_ref().unwrap(); }
}

#[derive(Default, Clone)]
pub struct AcpiHandler;

impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        PhysicalMapping::new(physical_address, NonNull::new(physical_address as *mut T).unwrap(), size, size, AcpiHandler)
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

pub fn init(rsdp_addr: usize) {
    let handler = AcpiHandler::default();

    unsafe {
        let tables = AcpiTables::from_rsdp(handler, rsdp_addr);
        match tables {
            Ok(tables) => {
                TABLES = Some(tables);
            }
            Err(_) => {
                panic!("Failed to parse ACPI tables");
            }
        }
    }
}