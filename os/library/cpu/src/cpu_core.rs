#![no_std]

#[cfg(target_arch = "x86_64")]
pub unsafe fn flush_cache(buffer: &[u8]) {
    let ptr = buffer.as_ptr();
    let len = buffer.len();
    let mut offset = 0;
    let cpuid = raw_cpuid::CpuId::new();
    let cache_line_size = cpuid
        .get_cache_parameters()
        .unwrap()
        .next()
        .map(|c| c.coherency_line_size())
        .unwrap_or(64); // default to 64 bytes if unavailable

    while offset < len {
        unsafe { core::arch::x86_64::_mm_clflush(ptr.add(offset) as *const _) }; // flush one cache line

        offset += cache_line_size;
    }
    unsafe { core::arch::x86_64::_mm_sfence() }; // ensure all flushes are globally visible
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn flush_cache(buffer: &[u8]) {
    // todo flush_cache for aarch64
}