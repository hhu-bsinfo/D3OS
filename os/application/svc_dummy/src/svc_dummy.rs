#![no_std]

extern crate alloc;

use alloc::string::String;
use concurrent::thread;
use naming::shared_types::{OpenOptions, SeekOrigin};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;
use time::systime;

// Sucht Zuweisungen mit = und gibt den Wert zurück, z.B. --sleep-ms=5000
fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    for arg in args {
        if let Some(v) = arg.strip_prefix(key) {
            return Some(v);
        }
    }
    None
}

// Sucht nach Flags ohne = und gibt dann true zurück, z.B. --quiet
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

#[unsafe(no_mangle)]
pub fn main() {
    let args: alloc::vec::Vec<String> = env::args().collect();

    let label = arg_value(&args, "--label=").unwrap_or("svc_dummy");
    let service_name = arg_value(&args, "--service-name=").unwrap_or(label);
    let heartbeat_path = arg_value(&args, "--heartbeat-path=").unwrap_or("/heartbeat");
    let print = has_flag(&args, "--print");
    let initial_sleep_ms: usize = arg_value(&args, "--initial-sleep-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_000);

    let sleep_ms: usize = arg_value(&args, "--sleep-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_000);

    let exit_after_ms: Option<usize> = arg_value(&args, "--exit-after-ms=")
        .and_then(|v| v.parse::<usize>().ok());

    let heartbeat_ms: Option<usize> = arg_value(&args, "--heartbeat-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&v| v != 0);

    let no_heartbeat = has_flag(&args, "--no-heartbeat");

    let hang_after_ms: Option<usize> = arg_value(&args, "--hang-after-ms=")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&v| v != 0);

    let panic_on_start = has_flag(&args, "--panic");
    let panic_after_ms: Option<usize> = arg_value(&args, "--panic-after-ms=")
        .and_then(|v| v.parse::<usize>().ok());

    println!("svc_dummy: start label={} service_name={} initial_sleep_ms={} sleep_ms={} heartbeat_ms={:?} hang_after_ms={:?}", label, service_name, initial_sleep_ms, sleep_ms, if no_heartbeat { None } else { heartbeat_ms }, hang_after_ms);

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

    let heartbeat_fh = if !no_heartbeat && heartbeat_ms.is_some() {     // TempFS Datei öffnen, um Nachrichten an den Service Manager zu senden
        match naming::open(heartbeat_path, OpenOptions::WRITEONLY) {
            Ok(fh) => Some(fh),
            Err(_) => None,
        }
    } 
    else {
        None
    };

    let mut elapsed_ms: usize = 0;

    loop {
        if let Some(ms) = hang_after_ms {
            if elapsed_ms >= ms {   // Extra hängen bleiben
                loop {
                    core::hint::spin_loop();
                }
            }
        }

        if let Some(fh) = heartbeat_fh {
            let heartbeat_value = systime().num_milliseconds() as u64;
            let _ = naming::seek(fh, 0, SeekOrigin::Start);
            let _ = naming::write(fh, &heartbeat_value.to_le_bytes());     // Heartbeat normal senden
        }

        if print {
            println!("svc_dummy: hi from {}", label);
        }

        let delay = if no_heartbeat {
            sleep_ms
        } 
        else {
            heartbeat_ms.unwrap_or(sleep_ms)
        };
        thread::sleep(delay);
        elapsed_ms = elapsed_ms.saturating_add(delay);
    }
}
