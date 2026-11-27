use acpi::PhysicalMapping;
use core::ptr::NonNull;


#[derive(Default, Clone)]
pub struct AcpiHandler;


impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        unsafe { PhysicalMapping::new(physical_address, NonNull::new(physical_address as *mut T).unwrap(), size, size, AcpiHandler) }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}
