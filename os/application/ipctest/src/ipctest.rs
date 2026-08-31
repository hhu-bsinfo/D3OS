#![no_std]
#![allow(unused_imports)]

extern crate alloc;

use alloc::string::ToString;
use alloc::{format, string::String, vec::Vec};
use alloc::vec;
use concurrent::{ipc::{self, MESSAGE_DATA_SIZE}, process, thread};
use runtime::*;
use terminal::println;
use uuid::Uuid;

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current();
    let thread = thread::current().unwrap();
    let receiver_pid = process.id().to_string();
    let receiver_tid = format!("{}", thread.id() + 1);

    println!("Hello from Thread [{}] in Process [{}]!\n", thread.id(), process.id());

    let mut args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        args = vec!["ipctest".to_string(), "start".to_string(), "SIMPLE".to_string(), "67".to_string()];
    }

    let command = args.get(1).unwrap();

    if command == "start" {

        let mut args_receive = Vec::new();
        args_receive.push("receive");
        thread::start_application("ipctest", args_receive);

        thread::sleep(1_000);

        let mut args_send = Vec::new();
        args_send.push("send");
        args_send.push(&receiver_pid);
        args_send.push(&receiver_tid);
        args_send.push(&args.get(2).map(|s| s.as_str()).unwrap_or("SIMPLE"));
        args.iter().skip(3).for_each(|s| args_send.push(s));
        thread::start_application("ipctest", args_send);

        return;
    } else if command == "send" {

        let target_pid = args.get(2).and_then(|s| Uuid::parse_str(&s).ok()).unwrap_or(Uuid::nil());
        let target_tid = args.get(3).and_then(|s| str::parse::<usize>(&s).ok()).unwrap_or(0);

        let args_numbers: Vec<_> = args.iter().skip(5)
            .map(|s| str::parse::<usize>(&s))
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .collect();

        let res = ipc::send(target_pid, target_tid, args_numbers.as_slice());

        //let res = ipc::send_small(Uuid::from_u128(0), target_tid, args.get(3).and_then(|s| str::parse::<u64>(&s).ok()).unwrap_or(42));
        match res {
            Ok(_) => println!("Send message successfully"),
            Err(err) => println!("Error sending message: {:?}", err),
        }
    } else if command == "receive" {
        ipc::init_queue();
        loop {
            match ipc::receive() {
                Some(msg) => {
                    println!("Received Message: {}", *msg);
                    break;
                },
                None => {}
            }
            thread::sleep(1_000);
        }
    } else if command == "pingpong" {

        let tid_start = thread.id() + 1;

        let num_pingpongs = 2;

        for i in 0..num_pingpongs {
            let d = (i + 1) % num_pingpongs;
            let tid_target = format!("{}", tid_start + d);
            thread::start_application("ipctest", vec!["pingpong_internal", &tid_target]);
        }
    } else if command == "pingpong_internal" {
        ipc::init_queue();
        ipc::send(Uuid::nil(), str::parse::<usize>(&args[2]).unwrap(), thread.id());
        loop {
            match ipc::receive() {
                Some(msg) => {
                    let ping = unsafe { (*(msg.data().as_ptr() as *const [usize; MESSAGE_DATA_SIZE / 8]))[0] };
                    println!("[{}] Received Message: {}", thread.id(), ping);
                    let pong = ping * 2;
                    ipc::send(Uuid::nil(), str::parse::<usize>(&args[2]).unwrap(), pong);
                },
                None => {}
            }
            thread::sleep(1_000);
        }
    } else if command == "loop" {
        ipc::init_queue();

        let mut args_send = Vec::new();
        args_send.push("loop_send");
        let current_pid = format!("{}", process.id());
        let current_tid = format!("{}", thread.id());
        args_send.push(&current_pid);
        args_send.push(&current_tid);
        let loop_send_tid = thread::start_application("ipctest", args_send).unwrap().id();

        let mut prev = 0;

        loop {
            while let Some(msg) = ipc::receive() {
                let number = *msg.data_as_usize();
                println!("Received Message: {}", number);
                if number == 100 {
                    println!("Successfully received 100 messages! Sending STOP signal to {}!", loop_send_tid);
                    ipc::send(Uuid::nil(), loop_send_tid, "STOP");
                    return;
                }
                if prev + 1 == number {
                    prev = number;
                } else {
                    println!("Error in message order! Sending STOP signal to {}!", loop_send_tid);
                    ipc::send(Uuid::nil(), loop_send_tid, "STOP");
                    return;
                }
            }
            thread::sleep(400);
        }
    } else if command == "loop_send" {

        let mut i = 1 as usize;

        let target_pid = args.get(2).and_then(|s| Uuid::parse_str(&s).ok()).unwrap_or(Uuid::nil());
        let target_tid = args.get(3).and_then(|s| str::parse::<usize>(&s).ok()).unwrap_or(0);
        let mut errors = 0;

        loop {

            // check for stop signal
            if let Some(msg) = ipc::receive() {
                
                let msg_tag = msg.tag();
                let msg_data = msg.data_as_str();

                if msg_tag == "TEXT" && msg_data == "STOP" {
                    println!("Received STOP signal ({} errors in total), exiting...", errors);
                    break;
                }
            }

            let res = ipc::send(target_pid, target_tid, i);
            match res {
                Ok(_) => {
                    println!("Send Message: {} with {} errors", i, errors);
                    i += 1;
                },
                //Err(err) => println!("Error sending message: {} [{:?}]", i, err),
                Err(_) => errors += 1,
            }
            thread::sleep(400);
        }
    }
}
