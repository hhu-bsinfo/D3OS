#![no_std]

extern crate alloc;

use alloc::string::ToString;
use alloc::{format, string::String, vec::Vec};
use alloc::vec;
use concurrent::{process, thread};
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
        args = vec!["ipctest".to_string(), "start".to_string(), "67".to_string()];
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
        args.iter().skip(2).for_each(|s| args_send.push(s));
        thread::start_application("ipctest", args_send);

        return;
    } else if command == "send" {

        let target_tid = args.get(2).and_then(|s| str::parse::<usize>(&s).ok()).unwrap_or(0);

        let args_u8: Vec<u8> = args.iter().skip(3)
            .map(|s| str::parse::<u8>(&s))
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .collect();

        let res = concurrent::ipc::send(Uuid::from_u128(0), target_tid, &args_u8);

        //let res = concurrent::ipc::send_small(Uuid::from_u128(0), target_tid, args.get(3).and_then(|s| str::parse::<u64>(&s).ok()).unwrap_or(42));
        match res {
            Ok(_) => println!("Send message successfully"),
            Err(err) => println!("Error sending message: {:?}", err),
        }
    } else {
        concurrent::ipc::init_queue();
        loop {
            match concurrent::ipc::receive() {
                Some(msg) => {
                    println!("Received Message: {:?}", msg.message());
                    break;
                },
                None => {}
            }
            thread::sleep(1_000);
        }
    }
}
