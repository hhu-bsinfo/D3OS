#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{println};
use vsock::{VsockAddr, VMADDR_CID_HOST};
use alloc::string::ToString;

#[unsafe(no_mangle)]
pub fn main() {
    println!("VSock Test Application started.");

    let host_addr = VsockAddr {
        cid: VMADDR_CID_HOST,
        port: 1234,
    };
    let local_port = 5000;

    println!("Connecting to host on port {}...", host_addr.port);

    // Verbindungsaufbau
    if let Err(e) = vsock::connect(host_addr, local_port) {
        println!("Connection failed: {:?}", e);
        return;
    }
    
    // Warten auf "Connected"-Ereignis (in einer einfachen Schleife)
    // In einer echten Anwendung würdest du eine bessere Event-Schleife verwenden.
    println!("Waiting for connection confirmation...");
    loop {
        if let Some(event) = vsock::poll() {
             match event.event_type {
                vsock::VsockEventType::Connected => {
                    println!("Successfully connected to host!");
                    break;
                },
                vsock::VsockEventType::Disconnected {..} => {
                    println!("Connection failed or was closed.");
                    return;
                }
                _ => {} // Andere Events ignorieren
             }
        }
        // Kurze Pause, um die CPU nicht zu überlasten
        syscall::sys_thread_sleep(10);
    }


    // Nachricht senden
    let message = "Hello from Guest!";
    println!("Sending message: '{}'", message);
    if let Err(e) = vsock::send(host_addr, local_port, message.as_bytes()) {
        println!("Send failed: {:?}", e);
        return;
    }
    
    // Auf Antwort warten
    println!("Waiting for reply...");
    let mut buffer = [0u8; 1024];
    loop {
        match vsock::recv(host_addr, local_port, &mut buffer) {
            Ok(len) if len > 0 => {
                let reply = core::str::from_utf8(&buffer[..len]).unwrap_or("Invalid UTF-8");
                println!("Received reply: '{}'", reply);
                break;
            }
            Ok(_) => {
                // Keine Daten verfügbar, weiter pollen
                syscall::sys_thread_sleep(10);
            },
            Err(e) => {
                println!("Receive failed: {:?}", e);
                break;
            }
        }
    }

    // Verbindung herunterfahren
    println!("Shutting down connection.");
    vsock::shutdown(host_addr, local_port).ok();
    
    println!("VSock Test Application finished.");
}