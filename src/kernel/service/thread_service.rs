use alloc::rc::Rc;
use x86_64::instructions::segmentation::{CS, DS, ES, FS, GS, Segment, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::PrivilegeLevel::Ring0;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use crate::kernel;
use crate::kernel::Service;
use crate::kernel::thread::scheduler::Scheduler;
use crate::kernel::thread::thread::Thread;

pub struct ThreadService {
    scheduler: Option<Scheduler>,
    gdt: GlobalDescriptorTable,
    tss: TaskStateSegment
}

impl Service for ThreadService {}

impl ThreadService {
    pub const fn new() -> Self {
        Self { scheduler: None, gdt: GlobalDescriptorTable::new(), tss: TaskStateSegment::new() }
    }

    pub fn init(&'static mut self) {
        self.scheduler = Some(Scheduler::new());

        // Setup global descriptor table
        self.gdt.add_entry(Descriptor::kernel_code_segment());
        self.gdt.add_entry(Descriptor::kernel_data_segment());
        self.gdt.add_entry(Descriptor::user_data_segment());
        self.gdt.add_entry(Descriptor::user_code_segment());
        self.gdt.add_entry(Descriptor::tss_segment(&self.tss));
        self.gdt.load();

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

    pub fn set_tss_rsp0(&mut self, rsp0: VirtAddr) {
        self.tss.privilege_stack_table[0] = rsp0;
    }

    pub fn start_scheduler(&mut self) {
        self.get_scheduler_mut().start();
    }

    pub fn ready_thread(&mut self, thread: Rc<Thread>) {
        self.get_scheduler_mut().ready(thread);
    }

    pub fn switch_thread(&mut self) {
        self.get_scheduler_mut().switch_thread();
    }

    pub fn sleep(&mut self, ms: usize) {
        self.get_scheduler_mut().sleep(ms);
    }

    pub fn set_scheduler_init(&mut self) {
        self.get_scheduler_mut().set_init();
    }

    pub fn get_current_thread(&self) -> Rc<Thread> {
        return self.get_scheduler_ref().get_current_thread();
    }

    pub fn exit_thread(&mut self) {
        self.get_scheduler_mut().exit();
    }

    pub fn join_thread(&mut self, thread_id: usize) {
        self.get_scheduler_mut().join(thread_id);
    }

    fn get_scheduler_ref(&self) -> &Scheduler {
        match self.scheduler.as_ref() {
            Some(scheduler) => scheduler,
            None => panic!("Thread Service: Trying to access scheduler before initialization!")
        }
    }

    fn get_scheduler_mut(&mut self) -> &mut Scheduler {
        match self.scheduler.as_mut() {
            Some(scheduler) => scheduler,
            None => panic!("Thread Service: Trying to access scheduler before initialization!")
        }
    }
}

#[no_mangle]
pub extern "C" fn tss_set_rsp0(rsp0: u64) {
    kernel::get_thread_service().set_tss_rsp0(VirtAddr::new(rsp0));
}