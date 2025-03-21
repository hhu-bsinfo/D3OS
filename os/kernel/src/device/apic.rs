use alloc::boxed::Box;
use alloc::format;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use acpi::madt::Madt;
use acpi::platform::interrupt::{InterruptSourceOverride, NmiSource, Polarity, TriggerMode};
use acpi::InterruptModel;
use alloc::vec::Vec;
use log::{info, warn};
use raw_cpuid::CpuId;
use spin::Mutex;
use x2apic::ioapic::{IoApic, IrqFlags, IrqMode, RedirectionTableEntry};
use x2apic::lapic::{LocalApic, LocalApicBuilder, TimerDivide, TimerMode};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, PageTableFlags};
use crate::{acpi_tables, allocator, interrupt_dispatcher, process_manager, scheduler, timer};
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::MemorySpace;
use crate::memory::vmm::VmaType;

pub struct Apic {
    local_apic: Mutex<LocalApic>,
    io_apics: Vec<(Mutex<IoApic>, u32)>, // (0: IO APIC instance, 1: Base Global System Interrupt)
    irq_overrides: Vec<InterruptSourceOverride>,
    nmi_sources: Vec<NmiSource>,
    timer_ticks_per_ms: usize
}

unsafe impl Send for Apic {}
unsafe impl Sync for Apic {}

#[derive(Default)]
struct ApicTimerInterruptHandler {}

impl InterruptHandler for ApicTimerInterruptHandler {
    fn trigger(&self) {
        scheduler().switch_thread_from_interrupt();
    }
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

                if features.has_x2apic() {
                    info!("X2Apic detected")
                } else {
                    info!("APIC detected");
                }
            }
        }


        // Find APIC relevant structures in ACPI tables
        let madt_mapping = acpi_tables().lock().find_table::<Madt>().expect("MADT not available");
        let madt = madt_mapping.get();
        let int_model = madt.parse_interrupt_model_in(allocator()).expect("Interrupt model not found in MADT");
        let cpu_info = int_model.1.expect("CPU info not found in interrupt model");

        info!("[{}] application {} detected", cpu_info.application_processors.len(), if cpu_info.application_processors.len() == 1 { "processor" } else { "processors" });
        info!("CPU [{}] is the bootstrap processor", cpu_info.boot_processor.processor_uid);
        
        // Vectors to store IRQ overrides and Non-maskable interrupts
        let mut irq_overrides = Vec::<InterruptSourceOverride>::new();
        let mut nmi_sources = Vec::<NmiSource>::new();
        
        // Vector to store initialized IO APICs with their base interrupt number
        let mut io_apics = Vec::<(Mutex<IoApic>, u32)>::new();
        
        // Create Local APIC instance
        let local_apic = Mutex::new(Self::create_local_apic(&madt));

        match int_model.0 {
            InterruptModel::Apic(apic_desc) => {
                // Read and store IRQ override entries
                info!("[{}] interrupt source {} detected", apic_desc.interrupt_source_overrides.len(), if apic_desc.interrupt_source_overrides.len() == 1 { "override" } else { "overrides" });

                for irq_override in apic_desc.interrupt_source_overrides.iter() {
                    info!("IRQ override [{}]->[{}], Polarity: [{:?}], Trigger: [{:?}]", irq_override.isa_source, irq_override.global_system_interrupt, irq_override.polarity, irq_override.trigger_mode);
                    irq_overrides.push(InterruptSourceOverride { isa_source: irq_override.isa_source, global_system_interrupt: irq_override.global_system_interrupt, polarity: irq_override.polarity, trigger_mode: irq_override.trigger_mode, });
                }

                // Read and store non-maskable interrupts sources
                info!("[{}] NMI {} detected", apic_desc.nmi_sources.len(), if apic_desc.nmi_sources.len() == 1 { "source" } else { "sources" });

                for nmi_source in apic_desc.nmi_sources.iter() {
                    info!("NMI source [{}], Polarity: [{:?}], Trigger: [{:?}]", nmi_source.global_system_interrupt, nmi_source.polarity, nmi_source.trigger_mode);
                    nmi_sources.push(NmiSource { global_system_interrupt: nmi_source.global_system_interrupt, polarity: nmi_source.polarity, trigger_mode: nmi_source.trigger_mode });
                }

                info!("[{}] IO {} detected", apic_desc.io_apics.len(), if apic_desc.io_apics.len() == 1 { "APIC" } else { "APICs" });

                // Iterate over IO APIC entries in MADT and initialize IO APICs (should only be a single one on most systems)
                for (i, io_apic_desc) in apic_desc.io_apics.iter().enumerate() {
                    info!("Initializing IO APIC [{}]", i);
                    let mut io_apic = Self::create_io_apic(io_apic_desc);

                    // Initialize redirection table with regards to IRQ override entries
                    // Needs to be executed in unsafe block; At this point, the IO APIC has been initialized successfully, so we can assume, that reading the MSR works.
                    let max_entry = io_apic_desc.global_system_interrupt_base + unsafe { io_apic.max_table_entry() } as u32;
                    info!("IO APIC [{}] handles interrupts [{}-{}]", i + 1, io_apic_desc.global_system_interrupt_base, max_entry);
                    
                    for i in io_apic_desc.global_system_interrupt_base..max_entry {
                        let mut entry = RedirectionTableEntry::default();
                        let mut flags = IrqFlags::MASKED;

                        entry.set_mode(IrqMode::Fixed);
                        entry.set_dest(cpu_info.boot_processor.local_apic_id as u8);

                        match override_for_target(&irq_overrides, i) {
                            None => entry.set_vector(i as u8 + InterruptVector::Pit as u8),
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
                        unsafe { io_apic.set_table_entry(i as u8, entry); }
                    }
                    
                    io_apics.push((Mutex::new(io_apic), io_apic_desc.global_system_interrupt_base));
                }
            },
            _ => panic!("No APIC described by MADT!"),
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

            // Needs to be executed in unsafe block; At this point, the APIC has been initialized successfully, so we can assume that reading the MSR works.
            entry.set_dest(unsafe { local_apic.lock().id() } as u8);

            // Find the correct IO APIC for the given NMI and set the corresponding entry in its redirection table
            match io_apic_for_target(&io_apics, nmi.global_system_interrupt) {
                Some(io_apic) => {
                    unsafe { io_apic.0.lock().set_table_entry(nmi.global_system_interrupt as u8, entry); }
                }, 
                None => warn!("No responsible IO APIC found for NMI [{}]", nmi.global_system_interrupt)
            }
        }

        // Initialization is finished -> Enable Local Apic
        unsafe {
            info!("Enabling Local APIC [{}]", cpu_info.boot_processor.local_apic_id);
            local_apic.lock().enable();
        }

        // Calibrate APIC timer
        let timer_ticks_per_ms = Apic::calibrate_timer(&mut local_apic.lock());
        info!("APIC Timer ticks per millisecond: [{}]", timer_ticks_per_ms);

        Self { local_apic, io_apics, irq_overrides, nmi_sources, timer_ticks_per_ms }
    }
    
    fn create_local_apic(madt: &Madt) -> LocalApic {
        // Read physical APIC MMIO base address and map it to the kernel address space
        let registers = Page::from_start_address(VirtAddr::new(madt.local_apic_address as u64)).expect("Local Apic MMIO address is not page aligned");
        process_manager().read().kernel_process().unwrap().virtual_address_space.map(PageRange { start: registers, end: registers + 1 }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE, VmaType::DeviceMemory, "lapic");
        
        LocalApicBuilder::new()
            .timer_vector(InterruptVector::ApicTimer as usize)
            .error_vector(InterruptVector::ApicError as usize)
            .spurious_vector(InterruptVector::Spurious as usize)
            .set_xapic_base(registers.start_address().as_u64())
            .build()
            .unwrap_or_else(|err| panic!("Failed to initialize Local APIC ({})!", err))
    }

    fn create_io_apic(io_apic_desc: &acpi::platform::interrupt::IoApic) -> IoApic {
        // Read physical IO APIC MMIO base address and map it to the kernel address space
        let registers = Page::from_start_address(VirtAddr::new(io_apic_desc.address as u64)).expect("IO Apic MMIO address is not page aligned");
        process_manager().read().kernel_process().unwrap().virtual_address_space.map(PageRange { start: registers, end: registers + 1 }, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE, VmaType::DeviceMemory, "ioapic");
        
        unsafe {
            let mut io_apic = IoApic::new(registers.start_address().as_u64());
            io_apic.init(io_apic_desc.global_system_interrupt_base as u8);

            io_apic
        }
    }
    
    pub fn allow(&self, vector: InterruptVector) {
        let target = target_gsi(&self.irq_overrides, vector as u8 - InterruptVector::Pit as u8);
        if is_nmi(&self.nmi_sources, target) {
            panic!("Trying to mask a non-maskable interrupt");
        }

        let io_apic = io_apic_for_target(&self.io_apics, target)
            .expect(format!("No responsible IO APIC found for interrupt [{}]", target).as_str());
        
        unsafe { io_apic.0.lock().enable_irq(target as u8); }
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

    pub fn start_timer(&self, interval_ms: usize) {
        let mut local_apic = self.local_apic.lock();

        unsafe {
            local_apic.set_timer_divide(TimerDivide::Div1);
            local_apic.set_timer_mode(TimerMode::Periodic);
            local_apic.set_timer_initial((self.timer_ticks_per_ms * interval_ms) as u32);
            local_apic.enable_timer();
        }

        interrupt_dispatcher().assign(InterruptVector::ApicTimer, Box::new(ApicTimerInterruptHandler::default()));
    }

    fn calibrate_timer(local_apic: &mut LocalApic) -> usize {
        unsafe {
            // Set APIC timer to count down from 0xffffffff
            local_apic.disable_timer();
            local_apic.set_timer_divide(TimerDivide::Div1);
            local_apic.set_timer_mode(TimerMode::OneShot);
            local_apic.set_timer_initial(0xffffffff);
            local_apic.enable_timer();

            // Wait 50 ms using the PIT
            timer().wait(50);

            // Calculate APIC timer ticks per millisecond
            let ticks_per_ms = ((0xffffffff - local_apic.timer_current()) / 50) as usize;
            local_apic.disable_timer();

            return ticks_per_ms;
        }
    }
}

fn target_gsi(irq_overrides: &Vec<InterruptSourceOverride>, source_irq: u8) -> u32 {
    match override_for_source(irq_overrides, source_irq) {
        None => source_irq as u32,
        Some(irq_override) => irq_override.global_system_interrupt,
    }
}

fn override_for_source(irq_overrides: &Vec<InterruptSourceOverride>, source_irq: u8) -> Option<&InterruptSourceOverride> {
    for irq_override in irq_overrides.iter() {
        if irq_override.isa_source == source_irq {
            return Some(irq_override);
        }
    }

    return None;
}

fn override_for_target(irq_overrides: &Vec<InterruptSourceOverride>, target_gsi: u32) -> Option<&InterruptSourceOverride> {
    for irq_override in irq_overrides.iter() {
        if irq_override.global_system_interrupt == target_gsi {
            return Some(irq_override);
        }
    }

    return None;
}

fn io_apic_for_target(io_apics: &Vec<(Mutex<IoApic>, u32)>, target_gsi: u32) -> Option<&(Mutex<IoApic>, u32)> {
    for entry in io_apics.iter() {
        let mut io_apic = entry.0.lock();
        let min_entry = entry.1;
        let max_entry = min_entry + unsafe { io_apic.max_table_entry() } as u32;
        
        if target_gsi >= min_entry && target_gsi <= max_entry {
            return Some(entry);
        }
    }
    
    None
}

fn is_nmi(nmi_sources: &Vec<NmiSource>, gsi: u32) -> bool {
    for nmi in nmi_sources.iter() {
        if nmi.global_system_interrupt == gsi {
            return true;
        }
    }

    return false;
}
