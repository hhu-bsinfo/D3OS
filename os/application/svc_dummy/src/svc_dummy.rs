#![no_std]

extern crate alloc;

use alloc::string::String;
use concurrent::thread;
use naming::shared_types::{OpenOptions, SeekOrigin};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;
use time::systime;
use core::net::SocketAddr;
use network::TcpStream;
use core::str::FromStr;
use alloc::format;
use log::{info, error};

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
    terminal::init_logger();
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

    let send_to_addr: Option<SocketAddr> = arg_value(&args, "--send-to=")
    .and_then(|v| SocketAddr::from_str(v).ok());

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

        // 2. NEU: Nachricht über das Netzwerk senden! (einmal)
        // Erfolgreich von QEMU zu Linux gesendet via lauschen mit nc -l -p 8080
        if let Some(target_addr) = send_to_addr {
        
        info!("svc_dummy: Versuche TCP Verbindung zu {}", target_addr);
        
        
        // TCP Verbindung aufbauen
        match TcpStream::connect(target_addr) {
            Ok(stream) => {
                //let msg = format!("Hello from D3OS Node! Mein Service heißt: {}\n", service_name);

                // Neuer Test: Nachricht an etcd senden
                // Base64! "test_a" = "dGVzdF9h", "online" = "b25saW5l"
                let payload = "{\"key\": \"dGVzdF9h\", \"value\": \"b25saW5l\"}";

                // HTTP/1.1 POST Request
                // \r\n sind zwingenden Zeilenumbrüche im HTTP-Protokoll
                let msg = format!("POST /v3/kv/put HTTP/1.1\r\nHost: 10.0.2.2:2379\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    payload.len(), payload);
                
                // Nachricht in den Socket schreiben
                match stream.write(msg.as_bytes()) {
                    Ok(bytes) => info!("svc_dummy: Erfolgreich {} Bytes an {} gesendet!", bytes, target_addr),
                    Err(e) => error!("svc_dummy: Fehler beim Senden: {:?}", e),
                }
            }
            Err(e) => {
                error!("svc_dummy: Konnte keine Verbindung aufbauen zu {}: {:?}", target_addr, e);
            }
        }
    }



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
