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

    println!("svc_dummy: start label={} initial_sleep_ms={} sleep_ms={}", label, initial_sleep_ms, sleep_ms);

    if initial_sleep_ms != 0 {
        thread::sleep(initial_sleep_ms);
    }

    loop {
        if !quiet {
            println!("svc_dummy: hi from {}", label);
        }
        thread::sleep(sleep_ms);
    }
}
