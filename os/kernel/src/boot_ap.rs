use log::info;
use x86_64::registers::control::{Cr4, Cr4Flags};
use crate::{APIC, apic, process_manager};
use crate::device::apic::Apic;
use crate::process::core_local_storage::{cls, cls_mut, init_gdt_for_this_core, install_gs_base, scheduler_start, scheduler};
use crate::process::{scheduler};
use crate::syscall::syscall_dispatcher;

/// First rust function called from assembly boot code for an application core
#[unsafe(no_mangle)]
pub extern "C" fn startup_ap(cpu_id: u32) {
    info!("    Application processor executing 'startup_ap'");

    unsafe {
        Cr4::update(|flags| flags.insert(Cr4Flags::FSGSBASE));
    }

    // installs the cpu_id in a cpuLocal struct on the GS segment
    install_gs_base(cpu_id);

    info!("Installed GS base for CPU {}", cpu_id);

    init_gdt_for_this_core();

    info!("Initialized GDT for CPU {}", cpu_id);

    cls_mut().init_apic(false);

    info!("Initialized apic for CPU {}", cpu_id);

    syscall_dispatcher::init();
    scheduler::cpu_mark_online();

    info!("Marked CPU {} online", cpu_id);

    // Wait until the bootstrap processor has fully initialized the global APIC
    let _apic_ref = APIC.wait();
    Apic::enable_local_apic(cls().local_apic());

    info!("Starting scheduler{}",cpu_id);
    apic().start_timer(10);
    scheduler_start();

    loop {}
}

/// Thread for testing multicore scheduling
#[allow(dead_code)]
pub(crate) extern "sysv64" fn debug_thread() {
    loop {
        info!("debug thread");
        scheduler().debug_scheduler();
        scheduler().sleep(1000);
    }
}