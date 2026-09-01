use alloc::string::String;
use uuid::Uuid;

use crate::process::process::Process;
use crate::memory::PAGE_SIZE;
pub struct ProcStat {
    pid: Uuid,
    name: String,
    utime: u64, // Time spent in User-Mode
    stime: u64, // Time spent in Kernel-Mode
    total_cpu_time: u64, // Total Time spent
    rss_user_pages: u64, // num-pages used
}

impl ProcStat {
    pub fn from_process(process: &Process) -> Self {
        Self {
            pid: process.id(),
            name: process.name().into(),
            utime: process.utime(),
            stime: process.stime(),
            total_cpu_time: process.utime() + process.stime(),
            rss_user_pages: process.rss_user_pages(),
        }
    }

    pub fn pid(&self) -> Uuid {
        self.pid
    }
    
    pub fn name(&self) -> &str {
        &self.name
    }
    
    
    // Time in User-Mode
    pub fn utime(&self) -> u64 {
        self.utime
    }
    // Time in Kernel-Mode
    pub fn stime(&self) -> u64 {
        self.stime
    }

    pub fn total_cpu_time(&self) -> u64 {
        self.total_cpu_time
    }

    pub fn rss_user_pages(&self) -> u64{
        self.rss_user_pages
    }

    pub fn rss_in_bytes(&self) -> u64{
        self.rss_user_pages * (PAGE_SIZE as u64)
    }
}