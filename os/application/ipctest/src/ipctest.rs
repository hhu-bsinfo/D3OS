#![no_std]
#![allow(unused_imports)]

extern crate alloc;

use core::fmt::Display;

use alloc::string::ToString;
use alloc::{format, string::String, vec::Vec};
use alloc::vec;
use concurrent::ipc::{ConstString, bytes_as_any};
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

        let res = ipc::send("ipctest_recv", "ipctest numbers", args_numbers.as_slice());
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

        // register self as pong
        ipc::register_ipc_endpoint("ipctest_pong");

        // start ping
        thread::start_application("ipctest", vec!["pingpong_ping"]);

        // wait for ping
        loop {
            if let Some(msg) = ipc::receive() {
                let ping = msg.data_as_usize();
                if *ping > 100 { return; }
                let pong = ping + 1;
                println!("PONG! [{}]", pong);
                ipc::answer_with_name(&msg, "pong", pong);
            }
            thread::sleep(200);
        }
    } else if command == "pingpong_ping" {
        // send first ping
        println!("PING! [{}]", 1);
        ipc::send("ipctest_pong", "ping", 1);

        // wait for pong
        loop {
            if let Some(msg) = ipc::receive() {
                let pong = msg.data_as_usize();
                if *pong > 100 { return; }
                let ping = pong + 1;
                println!("PING! [{}]", ping);
                ipc::answer_with_name(&msg, "ping", ping);
            }
            thread::sleep(200);
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
                    ipc::send("ipctest_send", "signal", "STOP");
                    return;
                }
                if prev + 1 == number {
                    prev = number;
                } else {
                    println!("Error in message order. Sending STOP signal and exiting...");
                    ipc::send("ipctest_send", "signal", "STOP");
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

            let res = ipc::send("ipctest_recv", "number", i);
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
    } else if command == "struct" {
        
        ipc::register_ipc_endpoint("ipctest_struct_recv");
        thread::start_application("ipctest", vec!["struct_send"]);

        thread::sleep(1500);

        loop {
            match ipc::receive() {
                Some(msg) => {
                    let my_struct: &MyStruct = unsafe { bytes_as_any(&*msg.data()) };
                    println!("-- Receive --\n{:}", my_struct);
                    break;
                },
                None => {}
            }
            thread::sleep(300);
        }

    } else if command == "struct_send" {
        let my_struct = MyStruct {
            a: 42,
            b: -67,
            c: ConstString::from("Hello World!"),
        };
        println!("-- Sender --\n{:}", my_struct);
        ipc::send_struct("ipctest_struct_recv", "test", &my_struct);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MyStruct {
    a: usize,
    b: i16,
    c: ConstString<32>,
}

impl Display for MyStruct {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "( a: {}, b: {}, c: {} )", self.a, self.b, self.c)
    }
}