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

    println!("Hello from Thread [{}] in Process [{}]", thread.id(), process.id());

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
        args.iter().skip(2).for_each(|s| args_send.push(s));
        thread::start_application("ipctest", args_send);

        return;
    } else if command == "send" {

        let args_numbers: Vec<_> = args.iter().skip(2)
            .map(|s| str::parse::<usize>(&s))
            .filter(|r| r.is_ok())
            .map(|r| r.unwrap())
            .collect();

        let res = ipc::send("ipctest_recv", args_numbers.as_slice());
        match res {
            Ok(_) => println!("Send message successfully"),
            Err(err) => println!("Error sending message: {:?}", err),
        }
    } else if command == "receive" {
        ipc::register_ipc_endpoint("ipctest_recv");
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
        let own_tid = thread.id();
        let target_tid = str::parse::<usize>(&args[2]).unwrap();
        ipc::register_ipc_endpoint(&format!("ipctest_recv_{}", own_tid));
        ipc::send(&format!("ipctest_recv_{}", target_tid), thread.id());
        loop {
            match ipc::receive() {
                Some(msg) => {
                    let ping = unsafe { (*(msg.data().as_ptr() as *const [usize; MESSAGE_DATA_SIZE / 8]))[0] };
                    println!("[{}] Received Message: {}", thread.id(), ping);
                    let pong = ping * 2;
                    ipc::send(&format!("ipctest_recv_{}", target_tid), pong);
                },
                None => {}
            }
            thread::sleep(1_000);
        }
    } else if command == "loop" {

        let limit = 50;

        ipc::register_ipc_endpoint("ipctest_recv");

        let mut args_send = Vec::new();
        args_send.push("loop_send");
        thread::start_application("ipctest", args_send);

        let mut prev = 0;

        loop {
            while let Some(msg) = ipc::receive() {
                let number = *msg.data_as_usize();
                println!("Received Message: {}", number);
                if number == limit {
                    println!("Successfully received {} messages. Sending STOP signal and exiting...", limit);
                    ipc::send("ipctest_send", "STOP");
                    return;
                }
                if prev + 1 == number {
                    prev = number;
                } else {
                    println!("Error in message order. Sending STOP signal and exiting...");
                    ipc::send("ipctest_send", "STOP");
                    return;
                }
            }
            thread::sleep(500);
        }
    } else if command == "loop_send" {

        ipc::register_ipc_endpoint("ipctest_send");

        let mut i = 1 as usize;
        let mut errors = 0;

        loop {

            // check for stop signal
            if let Some(msg) = ipc::receive() {
                
                let msg_data = msg.data_as_str();

                if msg_data == "STOP" {
                    println!("Received STOP signal ({} errors in total), exiting...", errors);
                    break;
                }
            }

            let res = ipc::send("ipctest_recv", i);
            match res {
                Ok(_) => {
                    println!("Send Message: {} with {} errors", i, errors);
                    i += 1;
                },
                //Err(err) => println!("Error sending message: {} [{:?}]", i, err),
                Err(_) => errors += 1,
            }
            thread::sleep(100);
        }
    }
}
