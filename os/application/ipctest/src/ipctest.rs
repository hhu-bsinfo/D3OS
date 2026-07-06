#![no_std]

extern crate alloc;

use core::num;

use alloc::{format, string::String, vec::{self, Vec}};
use concurrent::{ipc::IpcMessage, process, thread};
#[allow(unused_imports)]
use runtime::*;
use terminal::println;
use uuid::Uuid;

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current();
    let thread = thread::current().unwrap();
    let receiver_tid = format!("{}", thread.id() + 1);

    println!("Hello from Thread [{}] in Process [{}]!\n", thread.id(), process.id());

    let mut args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        println!("Missing required args");
    }

    let command = args.get(1).unwrap();


    if command == "start" {
        let mut args_receive = Vec::new();
        args_receive.push("receive");
        thread::start_application("ipctest", args_receive);
        thread::sleep(1_000);
        let mut args_send = Vec::new();
        args_send.push("send");
        args_send.push(&receiver_tid);
        for i in 1..4 {
            if let Some(arg) = args.get(i) {
                args_send.push(arg);
            }
        }
        thread::start_application("ipctest", args_send);
        return;
    } else if command == "send" {
        let target_tid = args.get(2).and_then(|s| str::parse::<usize>(&s).ok()).unwrap_or(0);
        let mut message_content = [0, 0, 0, 0];
        for i in 0..3 {
            let idx = 3 + i;
            message_content[i] = args.get(idx).and_then(|s| str::parse::<usize>(&s).ok()).unwrap_or(i * 9);
        }
        
        let message = IpcMessage::from_arr(&message_content);
        let res = concurrent::ipc::send(Uuid::from_u128(0), target_tid, &message);
        match res {
            Ok(_) => println!("Send message successfully"),
            Err(err) => println!("Error sending message: {:?}", err),
        }
    } else {
        concurrent::ipc::init_queue();
        let res = concurrent::ipc::receive_next_blocking(10_000);
        match res {
            Ok(message) => println!("Received Message: {:?}", message),
            Err(err) => println!("Error receiving message: {:?}", err),
        }
    }
}
