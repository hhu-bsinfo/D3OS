use alloc::boxed::Box;
use core::mem::{offset_of, ManuallyDrop};
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use raw_cpuid::CpuId;
use spin::{Mutex};
use thingbuf::mpsc::errors::TryRecvError;
use thingbuf::mpsc::Receiver;
use x2apic::lapic::LocalApic;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::PrivilegeLevel::Ring0;
use x86_64::registers::model_specific::KernelGsBase;
use x86_64::registers::segmentation::SegmentSelector;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use crate::device::apic::Apic;
use crate::process::scheduler::{per_cpu_apic_id, set_inbox_apic_id, Scheduler, MessageItem};
use crate::take_inbox_receiver;

const PREEMPT_COUNT_OFFSET: usize = offset_of!(CoreLocalStorage, preempt_count);

/// Core Local Storage.
/// Contains information, which is needed by the syscall handler.
/// The TSS address is never accessed directly, but via the swapgs instruction.
/// 'boot.rs' sets up the gs base register with a pointer to this struct for the boot processor.
/// Since multicore is implemented, we have one of these per core.
#[repr(C)]
pub struct CoreLocalStorage {
    self_ptr: *const CoreLocalStorage, //easy return through GS Segment with deref (with base)
    tss_rsp0_ptr: VirtAddr,
    user_rsp: VirtAddr,
    id: u32,
    local_apic: Option<Mutex<LocalApic>>,
    timer_ticks_per_ms: usize,  // currently unused => needs new calibration method
    tss: Mutex<TaskStateSegment>,
    gdt: Mutex<GlobalDescriptorTable>,
    scheduler: Scheduler,
    rx: Receiver<Option<MessageItem>>,  // single owner (this core)
    preempt_count: AtomicUsize,
}

impl CoreLocalStorage {
    pub fn new(id: u32) -> Self {
        Self {
            self_ptr: 0 as *const CoreLocalStorage,
            tss_rsp0_ptr: VirtAddr::zero(),
            user_rsp: VirtAddr::zero(),
            id,
            local_apic: None,
            timer_ticks_per_ms: 0,
            tss: Mutex::new(TaskStateSegment::new()),
            gdt: Mutex::new(GlobalDescriptorTable::new()),
            scheduler: Scheduler::new(),
            rx: take_inbox_receiver(id as usize),
            preempt_count: AtomicUsize::new(0),
        }
    }

    /// Returns a reference to the local_apic of this core.
    #[inline(always)]
    pub fn local_apic(&self) -> &Mutex<LocalApic> {
        &self.local_apic.as_ref().expect("Local Apic not initialized")
    }

    pub fn init_apic(&mut self, kernel_core: bool) {
        self.local_apic = Some(Apic::new_local_apic(kernel_core));
    }

    /// Tries to receive a MessageItem from the inbox.
    pub fn try_recv(&self) -> Result<Option<MessageItem>, TryRecvError> {
        self.rx.try_recv()
    }

    /// Sets the timer ticks per ms that will be used for the timer interrupt in the future.
    /// (needs to be fixed)
    pub fn set_timer_ticks_per_ms(&mut self, ticks: usize) {
        assert_ne!(ticks, 0);
        self.timer_ticks_per_ms = ticks;
    }

    /// Returns the timer ticks per ms that will be used for the timer interrupt.
    pub fn timer_ticks_per_ms(&self) -> usize {
        self.timer_ticks_per_ms
    }
}

/// Returns a new CoreLocalStorage Struct with static lifetime
fn create_core_local_storage(id: u32) -> *mut CoreLocalStorage {
    let core_local_storage = CoreLocalStorage::new(id);

    let cpu_local = Box::new(core_local_storage);
    let addr = Box::leak(cpu_local) as *mut CoreLocalStorage;
    unsafe { (*addr).self_ptr = addr; }
    addr as *mut CoreLocalStorage
}

/// Installs a Cpu Local Storage on the GS segment
pub fn install_gs_base(id: u32) {
    let core_local_ptr = create_core_local_storage(id);
    KernelGsBase::write(VirtAddr::from_ptr(core_local_ptr));
    set_inbox_apic_id(id as usize);    //sets the apic_id of the current core in the PER_CPU_SCHED Array
}

/// reads the IA32_KERNEL_GS_BASE MSR and returns the value as u64
/// does not switch gs bases but is slower
#[inline(always)]
fn read_kernel_gs_base() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
        "rdmsr",
        in("ecx") 0xC000_0102u32, // IA32_KERNEL_GS_BASE
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Returns the whole CLS from the current GS segment
#[inline(always)]
fn cls_ptr_from_gs() -> *mut CoreLocalStorage {
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
/// Returns the whole CLS from the switched kernelGS-Base
#[inline(always)]
pub fn cls_ptr() -> *mut CoreLocalStorage {
    with_kernel_gs( || { cls_ptr_from_gs()})
}

/// wraps the code of another method with "swapgs" calls to get access to the
/// kernelGS-Base instead of the GS-Base during execution
/// also saves the current IF and restores it afterwards
#[inline(always)]
pub fn with_kernel_gs<R>(f: impl FnOnce() -> R) -> R {
    unsafe {
        //save current rflags to restore interrupt flag afterwards
        let mut rflags: u64;
        core::arch::asm!(
        "pushfq",
        "pop {rflags}",
        rflags = out(reg) rflags,
        options(preserves_flags)
        );
        preempt_disable_no_swap();
        core::arch::asm!("cli", options(nomem, nostack));
        core::arch::asm!("swapgs", options(nomem, nostack, preserves_flags));

        let ret = f();

        // Swap GS back first, then restore IF if it was previously set
        core::arch::asm!("swapgs", options(nomem, nostack, preserves_flags));
        if (rflags & (1 << 9)) != 0 {
            core::arch::asm!("sti", options(nomem, nostack));
        }
        preempt_enable_no_swap();
        ret
    }
}




    //// Preemption Guard and accessors ////

/// Preemption Guard
pub struct ClsGuard<R> {
    _preempt: PreemptGuard,
    r: R,
}
impl<R> ClsGuard<R> {
    fn new(r: R) -> Self {
        let _preempt = PreemptGuard::new();
        Self { _preempt, r }
    }
}
impl<'a> Deref for ClsGuard<&'a CoreLocalStorage> {
    type Target = CoreLocalStorage;
    fn deref(&self) -> &CoreLocalStorage { self.r }
}
impl<'a> Deref for ClsGuard<&'a mut CoreLocalStorage> {
    type Target = CoreLocalStorage;
    fn deref(&self) -> &CoreLocalStorage { self.r }
}
impl<'a> DerefMut for ClsGuard<&'a mut CoreLocalStorage> {
    fn deref_mut(&mut self) -> &mut CoreLocalStorage { self.r }
}

/// Map a mutable CLS guard into a guard of one of its fields. (for future work)
impl<'a> ClsGuard<&'a CoreLocalStorage> {
    #[inline(always)]
    pub fn map_ref<T>(self, f: impl FnOnce(& CoreLocalStorage) -> & T) -> ClsGuard<&'a  T> {
        // Prevent `self` from being dropped (moving preempt out manually).
        let me = ManuallyDrop::new(self);

        // Move the preemption guard out (no drop of the old guard).
        let preempt = unsafe { ptr::read(&me._preempt) };
        // Getting the field reference.
        let sub: &T = f(me.r);

        // Building the new guard (keeping preemption disabled).
        ClsGuard { _preempt: preempt, r: sub }
    }
}
/*
pub type SchedulerRefGuard<'a> = ClsGuard<&'a Scheduler>;
#[inline(always)]     // sleep() needs to be modified for this to work
pub fn scheduler() -> SchedulerRefGuard<'static> {
    cls().map_ref(|c| &c.scheduler)
}*/

/// CLS getter with preemption guard.
pub fn cls() -> ClsGuard<&'static CoreLocalStorage> {
    let r = unsafe { & *cls_ptr() };
    ClsGuard::new(r)
}

/// mut CLS getter with preemption guard.
pub fn cls_mut() -> ClsGuard<&'static mut CoreLocalStorage> {
    let r = unsafe { &mut *cls_ptr() };
    ClsGuard::new(r)
}

pub struct PreemptGuard { /* !Send, !Sync; holds pinned state */ }
impl Drop for PreemptGuard {
    #[inline(always)]
    fn drop(&mut self) { preempt_enable_no_swap(); }
}

impl PreemptGuard {
    #[inline(always)]
    pub fn new() -> Self { preempt_disable_no_swap(); Self{} }
}

/// Disables preemption temporarily without switching gs bases.
#[inline(always)]
fn preempt_disable_no_swap() {
    let base = read_kernel_gs_base() as *mut u8;
    unsafe {
        let cnt_ptr = base.add(PREEMPT_COUNT_OFFSET) as *mut AtomicUsize;
        (*cnt_ptr).fetch_add(1, Ordering::SeqCst);
    }
}

/// Enables preemption temporarily without switching gs bases.
#[inline(always)]
fn preempt_enable_no_swap() {
    let base = read_kernel_gs_base() as *mut u8;
    let prev_counter;
    unsafe {
        let cnt_ptr = base.add(PREEMPT_COUNT_OFFSET) as *mut AtomicUsize;
        prev_counter = (*cnt_ptr).fetch_sub(1, Ordering::SeqCst);
    }
    debug_assert!(prev_counter > 0);
}

/// Returns true if preemption is currently disabled. (without switching gs bases)
#[inline(always)]
pub fn preempt_is_disabled() -> bool {
    let base = read_kernel_gs_base() as *mut u8;
    unsafe {
        let cnt_ptr = base.add(PREEMPT_COUNT_OFFSET) as *mut AtomicUsize;
        (*cnt_ptr).load(Ordering::SeqCst) != 0
    }
}




    //// Everything about the fields of the CLS ////

/// Returns the core id from the GS segment after switching the GS Segment back and forth
#[inline(always)]
pub fn current_core_id() -> u32 {
    with_kernel_gs( || { current_core_id_from_gs()})
}

/// Returns the core id from the CURRENT GS segment
#[inline(always)]
pub fn current_core_id_from_gs() -> u32 {
    let id: u32;
    unsafe {
        core::arch::asm!(
        "mov {tmp:e}, gs:[24]",    //id is after 3 pointers (3*8 bytes)
        tmp = out(reg) id,
        options(nostack, preserves_flags, readonly)
        );
    }
    id
}

/// Returns the APIC of this core.
#[inline(always)]
pub fn local_apic_static() -> Option<&'static Mutex<LocalApic>> {
    unsafe { (*cls_ptr()).local_apic.as_ref() }
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
    cls_mut().tss_rsp0_ptr = tss_rsp0_ptr;
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
        GS::set_reg(SegmentSelector::new(0, Ring0));
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
fn debug_cls() {

    let cls = cls();
    let tss_rsp0 = cls.tss_rsp0_ptr;
    let user_rsp = cls.user_rsp;
    let id = cls.id;
    let timer_ticks_per_ms = cls.timer_ticks_per_ms;

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