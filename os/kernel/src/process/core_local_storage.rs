use alloc::boxed::Box;
use core::mem::offset_of;
use core::ops::Deref;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use raw_cpuid::CpuId;
use spin::{Mutex, Once};
use thingbuf::mpsc::errors::TryRecvError;
use thingbuf::mpsc::Receiver;
use x2apic::lapic::LocalApic;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::PrivilegeLevel::Ring0;
use x86_64::registers::segmentation::{Segment64, SegmentSelector};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use crate::device::apic::Apic;
use crate::process::scheduler::{per_cpu_apic_id, set_inbox_apic_id, Scheduler, MessageItem};
use crate::take_inbox_receiver;

/// Core Local Storage
/// 
/// This struct contains information, which is needed eg. by the syscall handler.
/// It is quickly accessible via the (kernel's) GS register.
/// 'boot.rs' sets up the gs base register for the boot processor.
/// Since multicore is implemented, we have one of these per core.
#[repr(C)]
pub struct CoreLocalStorage {
    self_ptr: *const CoreLocalStorage, //easy return through GS Segment with deref (with base)
    tss_rsp0_ptr: VirtAddr,
    user_rsp: VirtAddr,
    id: u32,
    local_apic: Once<Mutex<LocalApic>>,
    timer_ticks_per_ms: AtomicUsize,  // currently unused => needs new calibration method
    tss: Mutex<TaskStateSegment>,
    gdt: Mutex<GlobalDescriptorTable>,
    scheduler: Scheduler,
    rx: Receiver<Option<MessageItem>>,  // single owner (this core)
}

impl CoreLocalStorage {
    pub fn new(id: u32) -> Self {
        Self {
            self_ptr: 0 as *const CoreLocalStorage,
            tss_rsp0_ptr: VirtAddr::zero(),
            user_rsp: VirtAddr::zero(),
            id,
            local_apic: Once::new(),
            timer_ticks_per_ms: AtomicUsize::default(),
            tss: Mutex::new(TaskStateSegment::new()),
            gdt: Mutex::new(GlobalDescriptorTable::new()),
            scheduler: Scheduler::new(),
            rx: take_inbox_receiver(id as usize),
        }
    }

    /// Returns a reference to the local_apic of this core.
    #[inline(always)]
    pub fn local_apic(&self) -> &Mutex<LocalApic> {
        &self.local_apic.get().expect("Local Apic not initialized")
    }

    pub fn init_apic(&self, is_bp: bool) {
        assert_eq!(is_bp, self.id == 0);
        self.local_apic.call_once(|| Mutex::new(Apic::new_local_apic(is_bp)));
    }

    /// Tries to receive a MessageItem from the inbox.
    pub fn try_recv(&self) -> Result<Option<MessageItem>, TryRecvError> {
        self.rx.try_recv()
    }

    /// Sets the timer ticks per ms that will be used for the timer interrupt in the future.
    /// (needs to be fixed)
    pub fn set_timer_ticks_per_ms(&self, ticks: usize) {
        assert_ne!(ticks, 0);
        self.timer_ticks_per_ms.store(ticks, Ordering::SeqCst);
    }

    /// Returns the timer ticks per ms that will be used for the timer interrupt.
    pub fn timer_ticks_per_ms(&self) -> usize {
        self.timer_ticks_per_ms.load(Ordering::SeqCst)
    }
}

/// Returns a new CoreLocalStorage Struct with static lifetime
fn create_core_local_storage(id: u32) -> *mut CoreLocalStorage {
    let core_local_storage = CoreLocalStorage::new(id);

    let cpu_local = Box::new(core_local_storage);
    let addr = Box::leak(cpu_local) as *mut CoreLocalStorage;
    unsafe { (*addr).self_ptr = addr; }
    addr
}

/// Installs a Cpu Local Storage on the GS segment
pub fn install_gs_base(id: u32) {
    let core_local_ptr = create_core_local_storage(id);
    unsafe { GS::write_base(VirtAddr::from_ptr(core_local_ptr)) };
    info!("gsbase for {}: {:?}", id, GS::read_base());
    set_inbox_apic_id(id as usize);    //sets the apic_id of the current core in the PER_CPU_SCHED Array
}

/// Returns the whole CLS from the current GS segment
#[inline(always)]
fn cls_ptr() -> *mut CoreLocalStorage {
    debug_assert!(!GS::read_base().is_null());
    let struct_ptr: u64;
    unsafe {
        core::arch::asm!(
        "mov {}, gs:[0]",
        out(reg) struct_ptr,
        options(nostack, preserves_flags)
        );
    }
    struct_ptr as *mut CoreLocalStorage
}

    //// Guard and accessors ////

/// Guard for the [`CoreLocalStorage`]
/// 
/// If a thread holds a reference to the core-local storage,
/// it can't be migrated to a different core.
pub struct ClsGuard {
    cls: &'static CoreLocalStorage,
}

impl ClsGuard {
    fn new(cls: &'static CoreLocalStorage) -> Self {
        // disable migration
        if let Some(t) = cls.scheduler.try_current_thread() {
            t.incr_cls_ref();
        }
        Self { cls }
    }
}

impl Drop for ClsGuard {
    fn drop(&mut self) {
        // re-enable migration
        if let Some(t) = self.cls.scheduler.try_current_thread() {
            t.decr_cls_ref();
        }
    }
}

impl Deref for ClsGuard {
    type Target = CoreLocalStorage;
    fn deref(&self) -> &CoreLocalStorage { &self.cls }
}

/// CLS getter with guard.
pub fn cls() -> ClsGuard {
    let r = unsafe { & *cls_ptr() };
    ClsGuard::new(r)
}

    //// Everything about the fields of the CLS ////

/// Returns the core id from the CURRENT GS segment
#[inline(always)]
pub fn current_core_id() -> u32 {
    let id: u32;
    unsafe {
        core::arch::asm!(
        "mov {tmp:e}, gs:[{offset}]",
        tmp = out(reg) id,
        offset = const offset_of!(CoreLocalStorage, id),
        options(nostack, preserves_flags, readonly)
        );
    }
    id
}

/// Returns the APIC of this core.
#[inline(always)]
pub fn local_apic_static() -> Option<&'static Mutex<LocalApic>> {
    unsafe { (*cls_ptr()).local_apic.get() }
}

/// Returns the Task State Segment of this core.
/// Needed to set up kernel/user mode switching.
#[inline(always)]
pub fn tss_static() -> &'static Mutex<TaskStateSegment> {
    // SAFETY: cls_ptr() points to a per-core CLS that is Box::leak'ed,
    // so its fields are stable for the kernel lifetime.
    // Caller must secure that preemption is disabled or temporarily impossible
    unsafe { &(*cls_ptr()).tss }
}

/// Initializes the per-core TSS by copying a fixed template layout.
pub fn init_tss_cls() {
    let tss_rsp0_ptr =
        VirtAddr::new(ptr::from_ref(tss_static().lock().deref()) as u64 + size_of::<u32>() as u64);
    unsafe { &mut *cls_ptr() }.tss_rsp0_ptr = tss_rsp0_ptr;
}

/// Returns the Global Descriptor Table of this core.
/// Needed to set up basic segmentation (flat model) and the TSS.
pub fn gdt_static() -> &'static Mutex<GlobalDescriptorTable> {
    unsafe { &(*cls_ptr()).gdt }
}

/// Initializes the per-core GDT by copying a fixed template layout.
/// Must be called on the current core after GS/CLS is installed and before enabling interrupts/scheduler.
pub fn init_gdt_for_this_core() {

    let mut gdt = gdt_static().lock();

    // Rebuild a new GDT
    gdt.append(Descriptor::kernel_code_segment()); // selector index 1
    gdt.append(Descriptor::kernel_data_segment()); // selector index 2
    gdt.append(Descriptor::user_data_segment());   // selector index 3
    gdt.append(Descriptor::user_code_segment());   // selector index 4

    unsafe {
        // We need to obtain a static reference to the TSS and GDT for the following operations.
        // We know, that they have a static lifetime, since they are declared as static variables in 'kernel/mod.rs'.
        // However, since they are hidden behind a Mutex, the borrow checker does not see them with a static lifetime.
        let gdt_ref = ptr::from_ref(gdt.deref()).as_ref().unwrap();
        let tss_ref = ptr::from_ref(tss_static().lock().deref()).as_ref().unwrap();
        gdt.append(Descriptor::tss_segment(tss_ref));
        gdt_ref.load();
    }

    unsafe {
        // Load task state segment
        load_tss(SegmentSelector::new(5, Ring0));

        // Set code and stack segment register
        CS::set_reg(SegmentSelector::new(1, Ring0));
        SS::set_reg(SegmentSelector::new(2, Ring0));

        // Other segment registers are not used in long mode (set to 0)
        DS::set_reg(SegmentSelector::new(0, Ring0));
        ES::set_reg(SegmentSelector::new(0, Ring0));
        FS::set_reg(SegmentSelector::new(0, Ring0));
        // GS is used for the CoreLocalStorage and already set correctly
    }
}

/// Scheduler.
/// Manages the execution of threads and switches between them.
/// Allows to access active threads, put threads to sleep, exit/kill threads and creates new ones.
#[inline(always)]
pub fn scheduler() -> &'static Scheduler {
    // SAFETY: cls_ptr() points to a per-core CLS that is Box::leak'ed,
    // so its fields are stable for the kernel lifetime.
    // Caller must secure that preemption is disabled or temporarily impossible
    unsafe { &(&*cls_ptr()).scheduler }
}

/// returns without starting if scheduler is already running
/// otherwise, does not return
#[inline(always)]
pub fn scheduler_start() {
    unsafe { (*cls_ptr()).scheduler.start(); }
}

/// Function for debugging cls specific information
#[allow(dead_code)]
fn debug_cls() {

    let cls = cls();
    let tss_rsp0 = cls.tss_rsp0_ptr;
    let user_rsp = cls.user_rsp;
    let id = cls.id;
    let timer_ticks_per_ms = cls.timer_ticks_per_ms.load(Ordering::SeqCst);

    if let Some(feat) = CpuId::new().get_feature_info() {
        let has_tsc = feat.has_tsc();
        let has_apic = feat.has_apic();
        let apic_id = feat.initial_local_apic_id();
        info!("\tNew Core {} going online:\n\tTSC: {}    APIC: {}    APIC-id: {}    \
            Ticks/ms: {}\n\tCLS_addr: {:p}    TSS_rsp0: {:p}    user_rsp: {:p}",
        id, has_tsc, has_apic, apic_id, timer_ticks_per_ms, cls_ptr(), tss_rsp0, user_rsp);
        info!(" Cpu ID should be {} and cpuLocal says {}", id, current_core_id());
        info!(" APIC ID should be {} and PER_CPU says {}", apic_id, per_cpu_apic_id(id as usize));
    }
    //info!("\t Local APIC:", local_apic.lock().deref());
    //cls.scheduler.debug_scheduler();
}