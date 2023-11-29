use alloc::format;
use alloc::vec::Vec;
use acpi::InterruptModel;
use acpi::madt::Madt;
use acpi::platform::interrupt::{InterruptSourceOverride, NmiSource, Polarity, TriggerMode};
use lazy_static::lazy_static;
use raw_cpuid::CpuId;
use spin::Mutex;
use x2apic::ioapic::{IoApic, IrqFlags, IrqMode, RedirectionTableEntry};
use x2apic::lapic::{LocalApic, LocalApicBuilder, xapic_base};
use crate::{kernel};
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::service::memory_service::AcpiAllocator;
use crate::kernel::log::Logger;

lazy_static!{
    static ref LOG: Logger = Logger::new("APIC");
}

pub struct Apic {
    local_apic: Option<Mutex<LocalApic>>,
    io_apic: Option<Mutex<IoApic>>,
    irq_overrides: Vec<InterruptSourceOverride>,
    nmi_sources: Vec<NmiSource>,
}

impl Apic {
    pub const fn new() -> Self {
        Self { local_apic: None, io_apic: None, irq_overrides: Vec::new(), nmi_sources: Vec::new() }
    }

    pub fn init(&mut self) {
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

        LOG.info("APIC detected!");

        // Find APIC relevant structures in ACPI tables
        let madt = kernel::get_device_service().get_acpi_tables().find_table::<Madt>().unwrap_or_else(|err| panic!("MADT not available ({:?})!", err));
        let int_model = madt.parse_interrupt_model_in(AcpiAllocator::new(kernel::get_memory_service())).unwrap_or_else(|err| panic!("Interrupt model not found in MADT ({:?})!", err));

        if let Some(cpu_info) = int_model.1 {
            LOG.info(format!("[{}] application {} detected", cpu_info.application_processors.len(), if cpu_info.application_processors.len() == 1 { "processor" } else { "processors" }).as_str());
            LOG.info(format!("CPU [{}] is the bootstrap processor", cpu_info.boot_processor.processor_uid).as_str());
        }

        // Read APIC MMIO base address and create new Local Apic instance
        // Needs to be executed in unsafe block; APIC availability has been check before hand, so this should work.
        let apic_address = unsafe { xapic_base() };
        self.local_apic = Some(Mutex::new(LocalApicBuilder::new()
            .timer_vector(InterruptVector::ApicTimer as usize)
            .error_vector(InterruptVector::ApicError as usize)
            .spurious_vector(InterruptVector::Spurious as usize)
            .set_xapic_base(apic_address)
            .build()
            .unwrap_or_else(|err| panic!("Failed to initialize Local APIC ({})!", err))));

        let mut local_apic = self.local_apic.as_mut().unwrap().lock();
        LOG.info(format!("Initialized local APIC [{}]", unsafe { local_apic.id() }).as_str());

        match int_model.0 {
            InterruptModel::Unknown => panic!("No APIC described by MADT!"),
            InterruptModel::Apic(apic_desc) => {
                LOG.info(format!("[{}] IO {} detected", apic_desc.io_apics.len(), if apic_desc.io_apics.len() == 1 { "APIC" } else { "APICs" }).as_str());

                if apic_desc.io_apics.len() > 1 {
                    panic!("More than one IO APIC found!");
                }

                let io_apic_desc = apic_desc.io_apics.get(0).unwrap_or_else(|| panic!("No IO APIC described by MADT!"));

                LOG.info("Initializing IO APIC");
                // Needs to be executed in unsafe block; Since exactly one IO APIC has been detected, this should work
                unsafe { self.io_apic = Some(Mutex::new(IoApic::new(io_apic_desc.address as u64))); }

                let mut io_apic = self.io_apic.as_mut().unwrap().lock();
                unsafe { io_apic.init(io_apic_desc.global_system_interrupt_base as u8); }

                // Read and store IRQ override entries
                LOG.info(format!("[{}] interrupt source {} detected", apic_desc.interrupt_source_overrides.len(), if apic_desc.interrupt_source_overrides.len() == 1 { "override" } else { "overrides" }).as_str());

                for irq_override in apic_desc.interrupt_source_overrides.iter() {
                    LOG.info(format!("IRQ override [{}]->[{}], Polarity: [{:?}], Trigger: [{:?}]", irq_override.isa_source, irq_override.global_system_interrupt, irq_override.polarity, irq_override.trigger_mode).as_str());
                    self.irq_overrides.push(InterruptSourceOverride {
                        isa_source: irq_override.isa_source,
                        global_system_interrupt: irq_override.global_system_interrupt,
                        polarity: irq_override.polarity,
                        trigger_mode: irq_override.trigger_mode });
                }

                // Read and store non-maskable interrupts sources
                LOG.info(format!("[{}] NMI {} detected", apic_desc.interrupt_source_overrides.len(), if apic_desc.interrupt_source_overrides.len() == 1 { "source" } else { "sources" }).as_str());

                for nmi_source in apic_desc.nmi_sources.iter() {
                    LOG.info(format!("NMI source [{}], Polarity: [{:?}], Trigger: [{:?}]", nmi_source.global_system_interrupt, nmi_source.polarity, nmi_source.trigger_mode).as_str());
                    self.nmi_sources.push(NmiSource {
                        global_system_interrupt: nmi_source.global_system_interrupt,
                        polarity: nmi_source.polarity,
                        trigger_mode: nmi_source.trigger_mode,
                    });
                }

                // Initialize redirection table with regards to IRQ override entries
                // Needs to be executed in unsafe block; At this point, the IO APIC has been initialized successfully, so we can assume, that reading the MSR works.
                for i in io_apic_desc.global_system_interrupt_base as u8 .. unsafe { io_apic.max_table_entry() } {
                    let mut entry = RedirectionTableEntry::default();
                    let mut flags = IrqFlags::MASKED;

                    entry.set_mode(IrqMode::Fixed);

                    // Needs to be executed in unsafe block; At this point, the APIC has been initialized successfully, so we can assume, that reading the MSR works.
                    entry.set_dest(unsafe { local_apic.id() } as u8);

                    match get_override_for_target(&self.irq_overrides, i) {
                        None => entry.set_vector(i + InterruptVector::Pit as u8),
                        Some(irq_override) => {
                            if irq_override.polarity == Polarity::ActiveLow {
                                flags |= IrqFlags::LOW_ACTIVE;
                            }
                            if irq_override.trigger_mode == TriggerMode::Level {
                                flags |= IrqFlags::LEVEL_TRIGGERED;
                            }

                            entry.set_vector(irq_override.isa_source + InterruptVector::Pit as u8);
                        }
                    }

                    entry.set_flags(flags);

                    // Needs to be executed in unsafe block; Tables entries have been initialized in IoApic::init(), so writing them works.
                    unsafe { io_apic.set_table_entry(i, entry); }
                }

                // Set entries for non-maskable interrupts
                for nmi in self.nmi_sources.iter() {
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
                    entry.set_dest(unsafe { local_apic.id() } as u8);

                    // Needs to be executed in unsafe block; Tables entries have been initialized in IoApic::init(), so writing them works.
                    unsafe { io_apic.set_table_entry(nmi.global_system_interrupt as u8, entry); }
                }
            },
            _ => panic!("No APIC described by MADT!")
        }

        // Initialization is finished -> Enable Local Apic
        LOG.info(format!("Enabling local APIC [{}]", unsafe { local_apic.id() }).as_str());
        unsafe {
            local_apic.enable();
            local_apic.disable_timer();
        }
    }

    pub fn allow(&mut self, vector: InterruptVector) {
        let target = get_target_gsi(&self.irq_overrides, vector as u8 - InterruptVector::Pit as u8);
        if is_nmi(&self.nmi_sources, target) {
            panic!("Trying to mask a non-maskable interrupt");
        }

        match self.io_apic.as_mut() {
            None => panic!("APIC: Trying to call allow() before init()!"),
            Some(io_apic) => unsafe { io_apic.lock().enable_irq(target); }
        }
    }

    pub fn send_eoi(&mut self, _vector: InterruptVector) {
        match self.local_apic.as_mut() {
            None => panic!("APIC: Trying to call send_eoi() before init()!"),
            Some(local_apic_mutex) => unsafe {
                let mut local_apic = local_apic_mutex.try_lock();
                while local_apic.is_none() {
                    local_apic_mutex.force_unlock();
                    local_apic = local_apic_mutex.try_lock();
                }

                local_apic.unwrap().end_of_interrupt();
            }
        }
    }
}

fn get_target_gsi(irq_overrides: &Vec<InterruptSourceOverride>, source_irq: u8) -> u8 {
    match get_override_for_source(irq_overrides, source_irq) {
        None => source_irq,
        Some(irq_override) => irq_override.global_system_interrupt as u8
    }
}

fn get_override_for_source(irq_overrides: &Vec<InterruptSourceOverride>, source_irq: u8) -> Option<&InterruptSourceOverride> {
    for irq_override in irq_overrides.iter() {
        if irq_override.isa_source == source_irq {
            return Some(irq_override);
        }
    }

    return None;
}

fn get_override_for_target(irq_overrides: &Vec<InterruptSourceOverride>, target_gsi: u8) -> Option<&InterruptSourceOverride> {
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
            return true
        }
    }

    return false
}