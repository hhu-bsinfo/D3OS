#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use concurrent::thread;
use concurrent::thread::Thread;
use core::arch::x86_64::{__m256i, _mm256_add_epi32, _mm256_setr_epi32, _mm256_store_si256};
use core::sync::atomic::AtomicBool;
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

static FAILED: AtomicBool = AtomicBool::new(false);

#[repr(align(32))]
struct Aligned32<T>(T);

#[unsafe(no_mangle)]
pub fn main() {
    if env::args().count() != 3 {
        panic!("Usage: avxtest <num_threads> <num_iterations>");
    }

    let num_threads = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    let num_iterations = env::args().nth(2).unwrap().parse::<usize>().unwrap();
    let tenth = num_iterations / 10;

    let mut threads = Vec::<Thread>::new();

    println!("Starting AVX test with {} threads and {} iterations", num_threads, num_iterations);

    for i in 1..num_threads + 1 {
        let thread = thread::create(move || {
            unsafe {
                for j in 1..num_iterations + 1 {
                    if FAILED.load(core::sync::atomic::Ordering::SeqCst) {
                        return;
                    }

                    let check = avx_test((i * 1) as i32, (i * 2) as i32, (i * 3) as i32, (i * 4) as i32);
                    if !check {
                        FAILED.store(true, core::sync::atomic::Ordering::SeqCst);
                        println!("\u{001b}[33mThread {}: Failed at iteration {}\u{001b}[39m", i, j);
                        return;
                    }

                    if j == 1 || j % tenth == 0 {
                        println!("Thread {}: {}%", i, (j * 100) / num_iterations);
                    }
                }
            }
        }).expect("Failed to create thread");

        threads.push(thread);
    }

    for thread in threads {
        let _ = thread.join();
    }

    if FAILED.load(core::sync::atomic::Ordering::SeqCst) {
        println!("\u{001b}[31mAVX test failed!\u{001b}[39m");
    } else {
        println!("\u{001b}[32mAVX test passed successfully!\u{001b}[39m");
    }
}

#[target_feature(enable = "avx2")]
/// Create two identical 256-bit vectors containing the values a, b, c, d, and add them together.
/// Afterward, check the result for plausibility and return the result.
/// This function is executed simultaneously by multiple threads.
/// If the kernel does not handle AVX context switching correctly, the check should fail sooner or later.
fn avx_test(a: i32, b: i32, c: i32, d: i32) -> bool {
    let va = _mm256_setr_epi32(a, b, c, d, a, b, c, d);
    let vb = _mm256_setr_epi32(a, b, c, d, a, b, c, d);
    let vres = _mm256_add_epi32(va, vb);

    unsafe {
        let mut res = Aligned32([0i32; 8]);
        _mm256_store_si256(res.0.as_mut_ptr() as *mut __m256i, vres);

        let res = res.0;
        res[0] == a + a && res[1] == b + b && res[2] == c + c && res[3] == d + d &&
        res[4] == a + a && res[5] == b + b && res[6] == c + c && res[7] == d + d
    }
}