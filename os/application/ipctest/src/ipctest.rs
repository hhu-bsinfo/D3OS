#![no_std]
#![allow(unused_imports)]

extern crate alloc;

use core::fmt::Display;
use core::sync::atomic::AtomicBool;

use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::{format, string::String, vec::Vec};
use alloc::vec;
use concurrent::{process, thread};
use d3os::const_string::ConstString;
use ipc::ipc_message::{IpcMessageData, IpcMessageType, IpcMessageWrapper};
use ipc::ipc_error::IpcError;
use runtime::*;
use syscall::syscall;
use terminal::println;
use uuid::Uuid;

static LOGGER: logger::Logger = logger::Logger::new();

#[unsafe(no_mangle)]
pub fn main() {
    let process = process::current();
    let thread = thread::current().unwrap();
    let tid = thread.id();

    log::set_logger(&LOGGER).map(|_| log::set_max_level(log::LevelFilter::Debug));
    log::info!("[{}:{}] Successfully initialized logger!", process.id(), tid);

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
            match ipc::receive("ipctest_recv") {
                Ok(msg_wrapper) => {
                    let m = msg_wrapper.msg();
                    println!("Received Message: {}", m);
                    break;
                },
                Err(err) => {
                    if err != IpcError::Empty {
                        println!("[{}] Error: {:?}", tid, err);
                    }
                },
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
            match ipc::receive("ipctest_pong") {
                Ok(msg) => {
                    let ping = msg.msg().data_as_usize_unchecked().unwrap();
                    if ping > 100 { return; }
                    let pong = ping + 1;
                    println!("PONG! [{}]", pong);
                    ipc::answer_with_name(&msg.msg(), "pong", pong);
                },
                Err(err) => println!("[{}] Error: {:?}", tid, err),
            }
            thread::sleep(200);
        }
    } else if command == "pingpong_ping" {
        // send first ping
        println!("PING! [{}]", 1);
        ipc::send("ipctest_pong", "ping", 1);

        // wait for pong
        loop {
            match ipc::receive_answer() {
                Ok(msg) => {
                    let pong = msg.msg().data_as_usize_unchecked().unwrap();
                    if pong > 100 { return; }
                    let ping = pong + 1;
                    println!("PING! [{}]", ping);
                    ipc::answer_with_name(&msg.msg(), "ping", ping);
                },
                Err(err) => println!("[{}] Error: {:?}", tid, err),
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
            while let Ok(msg) = ipc::receive("ipctest_recv") {
                let number = msg.msg().data_as_usize_unchecked().unwrap();
                println!("Received Message: {}", number);
                if number == limit {
                    println!("Successfully received {} messages. Sending STOP signal and exiting...", limit);
                    ipc::send_empty("ipctest_send", "STOP");
                    return;
                }
                if prev + 1 == number {
                    prev = number;
                } else {
                    println!("Error in message order. Sending STOP signal and exiting...");
                    ipc::send_empty("ipctest_send", "STOP");
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
            if let Ok(msg) = ipc::receive("ipctest_send") {

                if msg.msg().name_as_str() == "STOP" {
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
            match ipc::receive("ipctest_struct_recv") {
                Ok(msg) => {
                    let my_struct: &MyStruct = unsafe { ipc::bytes_as_any(&*msg.msg().data_as_bytes()) };
                    println!("-- Receive --\n{:}", my_struct);
                    break;
                },
                Err(err) => println!("[{}] Error: {:?}", tid, err),
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
    } else if command == "contention" || command == "c" {
        
        ipc::register_ipc_endpoint("ipctest_contention_recv");

        let num_child_processes = 4;

        thread::create(move || {
            for _ in 0..num_child_processes {
                thread::start_application("ipctest", vec!["contention_sender"]);
            }
        });

        thread::sleep(1000);

        let mut num_stop_req = 0;

        loop {
            match ipc::receive("ipctest_contention_recv") {
                Ok(msg_wrapper) => {
                    let msg = msg_wrapper.msg();
                    match msg.name_as_str() {
                        "STOP" => {
                            num_stop_req += 1;
                            log::info!("[{}] received STOP request from {} -- [{}/{}]", tid, msg.sender_tid(), num_stop_req, num_child_processes);
                            if num_stop_req >= num_child_processes {
                                log::info!("[{}] received {} STOP requests, exiting...", tid, num_stop_req);
                                return;
                            }
                        },
                        _ => {
                            if let Ok(data) = msg.data_as_usize_slice() && data.len() >= 2 {
                                log::info!("[{}] received request {} * {} from {}", tid, data[0], data[1], msg.sender_tid());
                                let res = data[0] * data[1];
                                log::info!("[{}] sending answer {} to {}", tid, res, msg.sender_tid());
                                ipc::answer(msg, res);
                            } else {
                                log::warn!("Invalid msg: {}", msg);
                            }
                        },
                    }
                },
                Err(err) => {
                    if err != IpcError::Empty {
                        log::error!("[{}] Error: {:?}", tid, err);
                    }
                },
            }
            thread::sleep(100);
        }
    } else if command == "contention_sender" {
        let mut a = tid;
        let b = 2;
        loop {
            log::info!("{} Sending {}*{}", tid, a, b);
            match ipc::send_and_receive_busy_polling("ipctest_contention_recv", "mult", &[a, b], 5000) {
                Ok(msg_wrapper) => {
                    a = msg_wrapper.msg().data_as_usize_unchecked().unwrap();
                    match a {
                        0 => {
                            log::error!("{} Error, received zero!", tid);
                        },
                        5000..usize::MAX => {
                            log::info!("{} Received {}", tid, a);
                            //ipc::answer_with_name(msg_wrapper.msg(), "CTRL", ());
                            ipc::send_empty("ipctest_contention_recv", "STOP");
                            println!("{} Done!", tid);
                            return;
                        },
                        _ => log::info!("{} Received {}", tid, a),
                    }
                },
                Err(err) => match err {
                    IpcError::Full => {
                        log::warn!("[{}] Full", tid);
                        thread::sleep(2000);
                    },
                    _ => log::error!("[{}] Error: {:?}", tid, err),
                },
            }
            thread::sleep(500);
        }
    } else if command == "mathserver" || command == "ms" {

        ipc::register_ipc_endpoint("mathserver_ctrl");

        let math_fns: [(&str, fn(f64, f64) -> f64); 4] = [
            ("mathserver_add", |i1, i2| i1 + i2),
            ("mathserver_sub", |i1, i2| i1 - i2),
            ("mathserver_mult", |i1, i2| i1 * i2),
            ("mathserver_div", |i1, i2| i1 / i2),
        ];

        for (endpoint_name, _) in math_fns {
            ipc::register_ipc_endpoint(endpoint_name);
        }

        let num_child_processes = 2;

        for _ in 0..num_child_processes {
            thread::start_application("ipctest", vec!["mathclient"]);
        }

        //thread::sleep(5000);

        let mut num_stop_req = 0;

        for (endpoint_name, math_fn) in math_fns {
            thread::create(move || {
                let tid = thread::current().unwrap().id();
                let debug_name = format!("[Mathserver - {}] {}", tid, endpoint_name);
                loop {
                    match ipc::receive_busy_polling(endpoint_name, 5000, 200) {
                        Ok(msg_wrapper) => {
                            let msg = msg_wrapper.msg();
                            log::info!("{} received {}", debug_name, msg);
                            let mut result: f64;
                            match msg.data() {
                                IpcMessageData::UsizeSlice { data, size } => {
                                    let numbers = &data[..*size];
                                    result = numbers[0] as f64;
                                    for n in numbers.iter().skip(1) {
                                        let f = *n as f64;
                                        result = (math_fn)(result, f);
                                    }
                                },
                                IpcMessageData::IsizeSlice { data, size } => {
                                    let numbers = &data[..*size];
                                    result = numbers[0] as f64;
                                    for n in numbers.iter().skip(1) {
                                        let f = *n as f64;
                                        result = (math_fn)(result, f);
                                    }
                                },
                                IpcMessageData::I32Slice { data, size } => {
                                    let numbers = &data[..*size];
                                    result = numbers[0] as f64;
                                    for n in numbers.iter().skip(1) {
                                        let f = *n as f64;
                                        result = (math_fn)(result, f);
                                    }
                                },
                                IpcMessageData::F64Slice { data, size } => {
                                    let numbers = &data[..*size];
                                    result = numbers[0];
                                    for n in numbers.iter().skip(1) {
                                        result = (math_fn)(result, *n);
                                    }
                                },
                                _ => {
                                    ipc::answer_with_name(msg, "ERROR", "Invalid Message Type!");
                                    continue;
                                },
                            }
                            log::info!("{} answer is {}", debug_name, result);
                            ipc::answer(msg, result);
                        },
                        Err(err) => {
                            if err != IpcError::Empty && err != IpcError::Timeout {
                                log::error!("[{}] Error: {:?}", tid, err);
                            }
                        },
                    }
                }
            });
        }

        loop {
            match ipc::receive_blocking("mathserver_ctrl", 5000) {
                Ok(msg_wrapper) => {
                    let msg = msg_wrapper.msg();
                    if msg.name() == "STOP" {
                        num_stop_req += 1;
                        log::info!("{} received STOP request from {} -- [{}/{}]", tid, msg.sender_tid(), num_stop_req, num_child_processes);
                        if num_stop_req >= num_child_processes {
                            log::info!("{} received [{}/{}] STOP requests, exiting...", tid, num_stop_req, num_child_processes);
                            return;
                        }
                    }
                },
                Err(err) => {
                    if err != IpcError::Empty {
                        log::error!("[{}] Error: {:?}", tid, err);
                    }
                },
            }
            thread::sleep(300);
        }
    } else if command == "mathclient" || command == "mc" {

        let math_fns = [
            "mathserver_add",
            "mathserver_sub",
            "mathserver_mult",
            "mathserver_div",
        ];
        let math_symb = [
            "+", "-", "*", "/",
        ];

        //let r = syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap();
        //let limit = (r * 41) % 13;
        let limit = 2;

        let mut count = 0;

        loop {
            let r1 = syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap();
            let r2 = syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap();
            let f = syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap();

            let ridx = (f * 7) % 4;
            let service_name = math_fns[ridx];

            let n1 = (r1 * 1009) % 999;
            let n2 = (r2 * 1093) % 999;

            match ipc::send_and_receive_busy_polling(service_name, "MATH", &[n1, n2], 5000) {
                Ok(msg_wrapper) => {
                    let res = msg_wrapper.msg().data_as_f64_unchecked();
                    println!("[{}] Sent {} {} {}, received {}", tid, n1, math_symb[ridx], n2, res);
                    count += 1;
                },
                Err(err) => {
                    log::error!("[{}] Error: {:?}", tid, err);
                },
            }

            if count >= limit {
                ipc::send_empty("mathserver_ctrl", "STOP");
                println!("[{}] Done", tid);
                return;
            }
        }
    } else {
        println!("Invalid argument {}", command)
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