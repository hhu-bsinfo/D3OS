#![no_std]

extern crate alloc;

use alloc::string::String;
use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    for arg in args {
        if let Some(v) = arg.strip_prefix(key) {
            return Some(v);
        }
    }
    None
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

#[unsafe(no_mangle)]
pub fn main() {
    let args: alloc::vec::Vec<String> = env::args().collect();

    let label = arg_value(&args, "--label=").unwrap_or("svc_dummy");
    let quiet = has_flag(&args, "--quiet") || has_flag(&args, "--no-print");
    let initial_sleep_ms: usize = arg_value(&args, "--initial-sleep-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_000);

    let sleep_ms: usize = arg_value(&args, "--sleep-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_000);

    let exit_after_ms: Option<usize> = arg_value(&args, "--exit-after-ms=")
        .and_then(|v| v.parse::<usize>().ok());

    let panic_on_start = has_flag(&args, "--panic") || has_flag(&args, "--crash");
    let panic_after_ms: Option<usize> = arg_value(&args, "--panic-after-ms=")
        .and_then(|v| v.parse::<usize>().ok());

    println!("svc_dummy: start label={} initial_sleep_ms={} sleep_ms={}", label, initial_sleep_ms, sleep_ms);

    if panic_on_start {
        panic!("svc_dummy: intentional crash (label={})", label);
    }

    if initial_sleep_ms != 0 {
        thread::sleep(initial_sleep_ms);
    }

    if let Some(ms) = panic_after_ms {
        if ms != 0 {
            thread::sleep(ms);
        }
        panic!("svc_dummy: intentional crash after {}ms (label={})", ms, label);
    }

    if let Some(ms) = exit_after_ms {
        if ms != 0 {
            thread::sleep(ms);
        }
        thread::exit_with_code(0);
    }

    loop {
        if !quiet {
            println!("svc_dummy: hi from {}", label);
        }
        thread::sleep(sleep_ms);
    }
}
