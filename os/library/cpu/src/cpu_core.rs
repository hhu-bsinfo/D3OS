#![no_std]

use raw_cpuid::CpuId;
use core::arch::x86_64::{_mm_clflush, _mm_sfence};

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn get_cache_line_size() -> usize {
    let cpuid = raw_cpuid::CpuId::new();
    cpuid
        .get_cache_parameters()
        .unwrap()
        .next()
        .map(|c| c.coherency_line_size())
        .unwrap_or(64) // default to 64 bytes if unavailable
}

pub unsafe fn flush_cache(buffer: &[u8]) {
    let ptr = buffer.as_ptr();
    let len = buffer.len();
    let mut offset = 0;
    #[cfg(target_arch = "x86_64")]
    let cache_line_size = get_cache_line_size();
    
    #[cfg(not(target_arch = "x86_64"))]
    let cache_line_size = 64;

    while offset < len {
        unsafe { _mm_clflush(ptr.add(offset) as *const _) }; // flush one cache line
        
        offset += cache_line_size;
    }
    unsafe { _mm_sfence() }; // ensure all flushes are globally visible
}