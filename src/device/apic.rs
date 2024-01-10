use crate::kernel;
use crate::kernel::memory::alloc::AcpiAllocator;
use crate::kernel::interrupt::interrupt_dispatcher::InterruptVector;
use acpi::madt::Madt;
use acpi::platform::interrupt::{InterruptSourceOverride, NmiSource, Polarity, TriggerMode};
use acpi::InterruptModel;
use alloc::vec::Vec;
use log::info;
use raw_cpuid::CpuId;
use spin::Mutex;
use x2apic::ioapic::{IoApic, IrqFlags, IrqMode, RedirectionTableEntry};
use x2apic::lapic::{xapic_base, LocalApic, LocalApicBuilder};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, PageTableFlags};
use crate::kernel::memory;
use crate::kernel::memory::MemorySpace;

pub struct Apic {
    local_apic: Mutex<LocalApic>,
    io_apic: Mutex<IoApic>,
    irq_overrides: Vec<InterruptSourceOverride>,
    nmi_sources: Vec<NmiSource>,
}

impl Apic {
    pub fn new() -> Self {
        // Check if APIC is available
        let cpuid = CpuId::new();
        match cpuid.get_feature_info() {
            None => panic!("APIC: Failed to read CPU ID features!"),
            Some(features) => {
                if !features.has_apic() {
                    panic!("APIC not available on this system!")
                }
            }
        }

        info!("APIC detected");

        // Find APIC relevant structures in ACPI tables
        let madt = kernel::acpi_tables().lock().find_table::<Madt>().expect("MADT not available!");
        let int_model = madt.parse_interrupt_model_in(AcpiAllocator::new(kernel::allocator())).expect("Interrupt model not found in MADT!");

        if let Some(cpu_info) = int_model.1 {
            info!("[{}] application {} detected", cpu_info.application_processors.len(), if cpu_info.application_processors.len() == 1 { "processor" } else { "processors" });
            info!("CPU [{}] is the bootstrap processor", cpu_info.boot_processor.processor_uid);
        }

        // Read physical APIC MMIO base address and map it to the kernel address space
        // Needs to be executed in unsafe block; APIC availability has been checked before, so this should work.
        let apic_page = Page::from_start_address(VirtAddr::new(unsafe { xapic_base() })).expect("Local Apic MMIO address is not page aligned!");
        memory::r#virtual::map(PageRange { start: apic_page, end: apic_page + 1 }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_CACHE);

        let local_apic = Mutex::new(LocalApicBuilder::new()
                .timer_vector(InterruptVector::ApicTimer as usize)
                .error_vector(InterruptVector::ApicError as usize)
                .spurious_vector(InterruptVector::Spurious as usize)
                .set_xapic_base(apic_page.start_address().as_u64())
                .build()
                .unwrap_or_else(|err| panic!("Failed to initialize Local APIC ({})!", err)),
        );

        let io_apic;
        let mut irq_overrides = Vec::<InterruptSourceOverride>::new();
        let mut nmi_sources = Vec::<NmiSource>::new();

        {
            let mut local_apic_locked = local_apic.lock();
            info!("Initialized local APIC [{}]", unsafe {
                local_apic_locked.id()
            });

            match int_model.0 {
                InterruptModel::Unknown => panic!("No APIC described by MADT!"),
                InterruptModel::Apic(apic_desc) => {
                    info!("[{}] IO {} detected", apic_desc.io_apics.len(), if apic_desc.io_apics.len() == 1 { "APIC" } else { "APICs" });

                    if apic_desc.io_apics.len() > 1 {
                        panic!("More than one IO APIC found!");
                    }

                    let io_apic_desc = apic_desc.io_apics.get(0).unwrap_or_else(|| panic!("No IO APIC described by MADT!"));

                    info!("Initializing IO APIC");
                    let io_apic_page = Page::from_start_address(VirtAddr::new(io_apic_desc.address as u64)).expect("IO Apic MMIO address is not page aligned!");
                    memory::r#virtual::map(PageRange { start: io_apic_page, end: io_apic_page + 1 }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_CACHE);
                    unsafe { io_apic = Mutex::new(IoApic::new(io_apic_page.start_address().as_u64())); } // Needs to be executed in unsafe block; Since exactly one IO APIC has been detected, this should work

                    let mut io_apic_locked = io_apic.lock();
                    unsafe { io_apic_locked.init(io_apic_desc.global_system_interrupt_base as u8); }

                    // Read and store IRQ override entries
                    info!(
                        "[{}] interrupt source {} detected", apic_desc.interrupt_source_overrides.len(), if apic_desc.interrupt_source_overrides.len() == 1 { "override" } else { "overrides" }
                    );

                    for irq_override in apic_desc.interrupt_source_overrides.iter() {
                        info!("IRQ override [{}]->[{}], Polarity: [{:?}], Trigger: [{:?}]", irq_override.isa_source, irq_override.global_system_interrupt, irq_override.polarity, irq_override.trigger_mode);
                        irq_overrides.push(InterruptSourceOverride { isa_source: irq_override.isa_source, global_system_interrupt: irq_override.global_system_interrupt, polarity: irq_override.polarity, trigger_mode: irq_override.trigger_mode, });
                    }

                    // Read and store non-maskable interrupts sources
                    info!("[{}] NMI {} detected", apic_desc.interrupt_source_overrides.len(), if apic_desc.interrupt_source_overrides.len() == 1 { "source" } else { "sources" });

                    for nmi_source in apic_desc.nmi_sources.iter() {
                        info!("NMI source [{}], Polarity: [{:?}], Trigger: [{:?}]", nmi_source.global_system_interrupt, nmi_source.polarity, nmi_source.trigger_mode);
                        nmi_sources.push(NmiSource { global_system_interrupt: nmi_source.global_system_interrupt, polarity: nmi_source.polarity, trigger_mode: nmi_source.trigger_mode });
                    }

                    // Initialize redirection table with regards to IRQ override entries
                    // Needs to be executed in unsafe block; At this point, the IO APIC has been initialized successfully, so we can assume, that reading the MSR works.
                    for i in io_apic_desc.global_system_interrupt_base as u8..unsafe { io_apic_locked.max_table_entry() } {
                        let mut entry = RedirectionTableEntry::default();
                        let mut flags = IrqFlags::MASKED;

                        entry.set_mode(IrqMode::Fixed);

                        // Needs to be executed in unsafe block; At this point, the APIC has been initialized successfully, so we can assume, that reading the MSR works.
                        entry.set_dest(unsafe { local_apic_locked.id() } as u8);

                        match override_for_target(&irq_overrides, i) {
                            None => entry.set_vector(i + InterruptVector::Pit as u8),
                            Some(irq_override) => {
                                if irq_override.polarity == Polarity::ActiveLow {
                                    flags |= IrqFlags::LOW_ACTIVE;
                                }
                                if irq_override.trigger_mode == TriggerMode::Level {
                                    flags |= IrqFlags::LEVEL_TRIGGERED;
                                }

                                entry.set_vector(
                                    irq_override.isa_source + InterruptVector::Pit as u8,
                                );
                            }
                        }

                        entry.set_flags(flags);

                        // Needs to be executed in unsafe block; Tables entries have been initialized in IoApic::init(), so writing them works.
                        unsafe {
                            io_apic_locked.set_table_entry(i, entry);
                        }
                    }

                    // Set entries for non-maskable interrupts
                    for nmi in nmi_sources.iter() {
                        let mut entry = RedirectionTableEntry::default();
                        let mut flags = IrqFlags::empty();

                        if nmi.polarity == Polarity::ActiveLow {
                            flags |= IrqFlags::LOW_ACTIVE;
                        }
                        if nmi.trigger_mode == TriggerMode::Level {
                            flags |= IrqFlags::LEVEL_TRIGGERED;
                        }

                        entry.set_mode(IrqMode::NonMaskable);
                        entry.set_vector(0);
                        entry.set_flags(flags);

                        // Needs to be executed in unsafe block; At this point, the APIC has been initialized successfully, so we can assume, that reading the MSR works.
                        entry.set_dest(unsafe { local_apic_locked.id() } as u8);

                        // Needs to be executed in unsafe block; Tables entries have been initialized in IoApic::init(), so writing them works.
                        unsafe {
                            io_apic_locked.set_table_entry(nmi.global_system_interrupt as u8, entry);
                        }
                    }
                }
                _ => panic!("No APIC described by MADT!"),
            }

            // Initialization is finished -> Enable Local Apic
            info!("Enabling local APIC [{}]", unsafe {
                local_apic_locked.id()
            });
            unsafe {
                local_apic_locked.enable();
                local_apic_locked.disable_timer();
            }
        }

        return Self {
            local_apic,
            io_apic,
            irq_overrides,
            nmi_sources,
        };
    }

    pub fn allow(&self, vector: InterruptVector) {
        let target = target_gsi(&self.irq_overrides, vector as u8 - InterruptVector::Pit as u8);
        if is_nmi(&self.nmi_sources, target) {
            panic!("Trying to mask a non-maskable interrupt");
        }

        unsafe { self.io_apic.lock().enable_irq(target); }
    }

    pub fn end_of_interrupt(&self) {
        let mut local_apic = self.local_apic.try_lock();
        while local_apic.is_none() {
            // It its extremely unlikely, that the local APIC is locked during an interrupt,
            // but if it happens, the whole system would hang, trying to send an EOI.
            unsafe { self.local_apic.force_unlock(); }
            local_apic = self.local_apic.try_lock();
        }

        unsafe { local_apic.unwrap().end_of_interrupt(); }
    }
}

fn target_gsi(irq_overrides: &Vec<InterruptSourceOverride>, source_irq: u8) -> u8 {
    match override_for_source(irq_overrides, source_irq) {
        None => source_irq,
        Some(irq_override) => irq_override.global_system_interrupt as u8,
    }
}

fn override_for_source(
    irq_overrides: &Vec<InterruptSourceOverride>,
    source_irq: u8,
) -> Option<&InterruptSourceOverride> {
    for irq_override in irq_overrides.iter() {
        if irq_override.isa_source == source_irq {
            return Some(irq_override);
        }
    }

    return None;
}

fn override_for_target(
    irq_overrides: &Vec<InterruptSourceOverride>,
    target_gsi: u8,
) -> Option<&InterruptSourceOverride> {
    for irq_override in irq_overrides.iter() {
        if irq_override.global_system_interrupt as u8 == target_gsi {
            return Some(irq_override);
        }
    }

    return None;
}

fn is_nmi(nmi_sources: &Vec<NmiSource>, gsi: u8) -> bool {
    for nmi in nmi_sources.iter() {
        if nmi.global_system_interrupt == gsi as u32 {
            return true;
        }
    }

    return false;
}
