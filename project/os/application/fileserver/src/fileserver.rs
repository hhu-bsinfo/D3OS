#![no_std]

extern crate alloc;

use terminal::print;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::result::Result::Ok;
use core::result::Result;
use core::option::Option::*;
use concurrent::thread;
use concurrent::thread::{create, sleep};
use naming::{mkfifo, open, read, write, ROOT, SHARED_PIPE};
use naming::shared_types::{OpenOptions, Capability};
use syscall::return_vals::Errno;

#[allow(unused_imports)]
use runtime::*;
use terminal::{println};

type FileHandle = usize;

enum Command {
    Write { content: Vec<u8> },
    Read { handle: FileHandle },
}

enum Response {
    Handle(FileHandle),
    Content(Vec<u8>),
    Error,
}

pub struct FileServer {
    files: BTreeMap<FileHandle, u8>,
    next_handle: usize,
    command_pipe: Capability,
}

impl FileServer {
    pub(crate) fn new(pipe: Capability) -> Result<Self, Errno> {
        Ok(Self {
            files: BTreeMap::new(),
            next_handle: 1,
            command_pipe: pipe,
        })
    }

    pub(crate) fn run(&mut self) -> Result<(), Errno> {
        let mut cmd_buf = [0u8; 2]; // 1 byte command + 8 bytes data

        println!("sending ack to client");

        let Ok(openpipe) = open(self.command_pipe, OpenOptions::READWRITE) else {
            println!("Failed to open command pipe");
            return Err(Errno::EUNKN);
        };

        let _ = write(openpipe, &[1u8]); // Send ACK to client

        println!("File server waiting for command...");

        loop {
            // Read command
            let _ = read(openpipe, &mut cmd_buf);
            println!("File server received command: {}", cmd_buf[0]);

            match cmd_buf[0] {
                // Write command
                1 => {
                    let letter:u8 = cmd_buf[1].clone();
                    let handle = self.next_handle;
                    self.next_handle += 1;
                    self.files.insert(handle, letter.clone());

                    let response = handle;
                    write(openpipe, &[response as u8])?;
                    println!("File server stored letter '{}' with handle {}", letter.clone() as char, handle);

                    sleep(2000); // Sleep a bit to ensure the client has processed the response
                }

                // Read command
                2 => { // Read
                    let handle = cmd_buf[1].clone() as usize;
                    if let Some(&letter) = self.files.get(&handle) {
                        write(openpipe, &[letter as u8])?;
                        println!("File server sent letter '{}' for handle {}", letter, handle);
                    } else {
                        write(openpipe, &[0])?;
                        println!("File server: handle {} not found", handle);
                    }
                    break; //enough for this demo
                }



                _ => {} // Invalid command
            }
        }
        Ok(())
    }
}

fn server_thread1() {
    let cap_pos = 2; //this wont work if multiple naming caps are added quickly
    let mut server = FileServer::new(Capability::new(cap_pos)).unwrap();

    println!("File server thread started for pipe at {}", cap_pos);
    if let Err(e) = server.run() {
        println!("File server error: {:?}", e);
    }

}
fn server_thread2() {
    let cap_pos = 3; //this wont work if multiple naming caps are added quickly
    let mut server = FileServer::new(Capability::new(cap_pos)).unwrap();

    println!("File server thread started for pipe at {}", cap_pos);
    if let Err(e) = server.run() {
        println!("File server error: {:?}", e);
    }

}

fn monitor_thread() {
    let id = thread::current().unwrap().id() as u8;

    let naming_len = 2;
    let Ok(open_shared_pipe) = open(SHARED_PIPE, OpenOptions::READWRITE) else {
        println!("Failed to open shared pipe in monitor thread");
        return;
    };

    for i in 0..10 { //do more if I want to connect more clients
        write(open_shared_pipe, &[id]);
    }

    sleep(3000); // Sleep a bit to ensure the new naming capability is registered and available

    loop {
        if capabilities::get_naming_len() > naming_len {
            //create a server thread
            let _ = create(server_thread1);
            println!("Monitor thread: Detected new client, started server thread");
            

            sleep(10000);
            let _ = create(server_thread2);
            println!("Monitor thread: Detected new client, started server thread");
            
            break;
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() -> () {
    // Create the monitoring thread
    let _ = create(monitor_thread);
    let Some(t1) = thread::start_application("fileclient1", Vec::new()) else {;
        println!("Failed to start file client");
        return;
    };
    let Some(t2) = thread::start_application("fileclient2", Vec::new()) else {;
        println!("Failed to start file client");
        return;
    };

    loop{}
}

// Idealerweise sollte über 2 pipes kommuniziert werden. Eine zum Lesen, eine zum schreiben,
// damit der server/client nicht die eigene anfrage/antwort liest und so direkt entfernt