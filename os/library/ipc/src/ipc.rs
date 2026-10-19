#![no_std]
#![feature(generic_atomic)] // used by lfqueue
#![feature(integer_casts)]
#![allow(clippy::new_without_default)]

pub mod ipc_message;
pub mod ipc_message_numbers;
pub mod ipc_error;
pub mod queues;

extern crate alloc;

use core::sync::atomic::AtomicBool;

use alloc::format;
use spin::RwLock;
use syscall::{SystemCall, syscall};

use concurrent::{process, thread};

use d3os::const_string::ConstString;
use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageData, IpcMessageType, IpcMessageWrapper};
use crate::ipc_error::IpcError;
use crate::queues::{IpcQueue, IpcQueueType};

// Schnittstelle für Interprozesskommunikation

const MESSAGE_NAME_LEN: usize = 0x20;
//pub const MESSAGE_DATA_SIZE: usize = 0x100 - (size_of::<Uuid>() + size_of::<usize>() + MESSAGE_TAG_LEN);
pub const MESSAGE_DATA_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 16;

const ENDPOINT_NAME_LEN: usize = 64;
const ENDPOINT_NUM: usize = 8;

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

static ENDPOINTS_INIT: AtomicBool = AtomicBool::new(false);

//pub type EndpointEntry = (ConstString<64>, *const IpcQueueType);

pub struct IpcEndpoints {
    endpoints: [RwLock<ConstString<ENDPOINT_NAME_LEN>>; ENDPOINT_NUM],
    queues: [IpcQueueType; ENDPOINT_NUM]
}

impl IpcEndpoints {
    pub fn new() -> Self {
        Self {
            endpoints: [const { RwLock::new(ConstString::new()) }; _],
            queues: core::array::from_fn(|_| IpcQueueType::new())
        }
    }
    pub fn get(&self, name: &str) -> Option<&IpcQueueType> {
        let pos = self.endpoints.iter().position(|endpoint_name| endpoint_name.read().as_str_unchecked() == name)?;
        Some(&self.queues[pos])
    }
    pub fn try_register(&self, svc_name: &str) -> Result<usize, IpcError> {

        let opt_duplicate = self.endpoints.iter().find(|name| name.read().as_str_unchecked() == svc_name);

        if opt_duplicate.is_some() {
            return Err(IpcError::Invalid);
        }

        match self.endpoints.iter().position(|name| name.read().is_empty()) {
            Some(pos) => {
                self.endpoints[pos].write().set(svc_name);
                Ok(pos)
            },
            None => Err(IpcError::Full),
        }
    }
}

/// registers an endpoint with the given name \
/// returns an error if the endpoint already exists
pub fn register_ipc_endpoint(endpoint_name: &str) -> Result<usize, IpcError> {
    // TODO what happens when we register multiple endpoints / multiple services for this endpoint

    let endpoints_ptr = USER_SPACE_MAILBOX_START as *mut IpcEndpoints;
    // if queue is not initialized, create new queue
    if ENDPOINTS_INIT.compare_exchange(false, true, core::sync::atomic::Ordering::Relaxed, core::sync::atomic::Ordering::Relaxed).is_ok() {
        unsafe { endpoints_ptr.write(IpcEndpoints::new()) };
    }

    // try creating a new endpoint
    unsafe { (*endpoints_ptr).try_register(endpoint_name)?; }

    let name_ptr = endpoint_name.as_ptr();
    let name_len = endpoint_name.len();
    IpcError::convert_syscall_result(syscall(SystemCall::IpcRegister, &[name_ptr as usize, name_len]))
}

pub fn remove_ipc_endpoint(svc_name: &str) -> Result<usize, IpcError> {
    let name_ptr = svc_name.as_ptr();
    let name_len = svc_name.len();
    IpcError::convert_syscall_result(syscall(SystemCall::IpcDelete, &[name_ptr as usize, name_len]))
}

pub fn create_msg<T: IpcMessageContent>(content: T) -> IpcMessageData {
    content.to_data()
}

/// send message content directly
/// 
/// this copies the given data, to create a new IpcMessageData struct
pub fn send<T: IpcMessageContent>(service: &str, msg_name: &str, content: T) -> Result<usize, IpcError> {
    send_internal(service, msg_name, content.msg_type(), content.to_bytes(), &format!("callback-{}", process::current().id()))
}
pub fn send_with_callback<T: IpcMessageContent>(service: &str, msg_name: &str, callback: &str, content: T) -> Result<usize, IpcError> {
    send_internal(service, msg_name, content.msg_type(), content.to_bytes(), callback)
}

pub fn send_empty(service: &str, msg_name: &str) -> Result<usize, IpcError> {
    send_internal(service, msg_name, IpcMessageType::Empty, &[], &format!("callback-{}", process::current().id()))
}

/// send a custom data structure over ipc
pub fn send_struct<T: Copy>(service: &str, msg_name: &str, message: &T) -> Result<usize, IpcError> {
    let bytes = unsafe { any_as_bytes(message) };
    send_internal(service, msg_name, IpcMessageType::Struct, bytes, &format!("callback-{}", process::current().id()))
}

/// send message and block waiting for answer
pub fn send_and_receive<T: IpcMessageContent>(service: &str, msg_name: &str, content: T, timeout_ms: usize) -> Result<impl IpcMessageWrapper, IpcError> {
    let callback_endpoint_name = format!("callback-{}", process::current().id());
    match send_internal(service, msg_name, content.msg_type(), content.to_bytes(), &callback_endpoint_name) {
        Ok(_) => {
            let mut counter = 0;
            loop {
                let rec = receive_blocking(&callback_endpoint_name, timeout_ms);
                match rec {
                    // TODO find specific answer message
                    Ok(_) => return rec,
                    Err(err) => {
                        match err {
                            IpcError::Empty => {
                                thread::sleep(200);
                                counter += 200;
                                if counter >= timeout_ms {
                                    return Err(IpcError::Timeout);
                                }
                            },
                            _ => return Err(err),
                        }
                    },
                }
            }
        },
        Err(err) => Err(err),
    }
}

/// send message and wait for answer
pub fn send_and_receive_busy_polling<T: IpcMessageContent>(service: &str, msg_name: &str, content: T, timeout_ms: usize) -> Result<impl IpcMessageWrapper, IpcError> {
    let callback_endpoint_name = format!("callback-{}", process::current().id());
    match send_internal(service, msg_name, content.msg_type(), content.to_bytes(), &callback_endpoint_name) {
        Ok(_) => {
            let mut counter = 0;
            loop {
                let rec = receive_busy_polling(&callback_endpoint_name, timeout_ms, 100);
                match rec {
                    // TODO find specific answer message
                    Ok(_) => return rec,
                    Err(err) => {
                        match err {
                            IpcError::Empty => {
                                thread::sleep(200);
                                counter += 200;
                                if counter >= timeout_ms {
                                    return Err(IpcError::Timeout);
                                }
                            },
                            _ => return Err(err),
                        }
                    },
                }
            }
        },
        Err(err) => Err(err),
    }
}

pub fn answer<T: IpcMessageContent>(original_message: &IpcMessage, answer_name: &str, content: T) -> Result<usize, IpcError> {
    send_internal(
        original_message.callback.as_str_unchecked(), 
        answer_name,
        content.msg_type(), 
        content.to_bytes(), 
        &format!("callback-{}", process::current().id())
    )
}

pub fn answer_with_callback<T: IpcMessageContent>(original_message: &IpcMessage, answer_name: &str, callback: &str, content: T) -> Result<usize, IpcError> {
    send_internal(
        original_message.callback.as_str_unchecked(), 
        answer_name,
        content.msg_type(), 
        content.to_bytes(), 
        callback
    )
}

pub struct IpcSendSyscallParams {
    pub target_endpoint_name: (*const u8, usize),
    pub msg_name: (*const u8, usize),
    pub msg_type: IpcMessageType,
    pub data: (*const u8, usize),
    pub callback: (*const u8, usize)
}

pub fn send_internal(target_endpoint: &str, msg_name: &str, msg_type: IpcMessageType, data: &[u8], callback: &str) -> Result<usize, IpcError> {

    // register callback endpoint
    let _res = register_ipc_endpoint(callback);

    let params = IpcSendSyscallParams {
        target_endpoint_name: (target_endpoint.as_ptr(), target_endpoint.len()),
        msg_name: (msg_name.as_ptr(), msg_name.len()),
        msg_type,
        data: (data.as_ptr(), data.len()),
        callback: (callback.as_ptr(), callback.len())
    };

    IpcError::convert_syscall_result(syscall(SystemCall::IpcSend, &&params))
}

pub fn receive(endpoint_name: &str) -> Result<impl IpcMessageWrapper + use<>, IpcError> {
    let endpoints = unsafe { (USER_SPACE_MAILBOX_START as *const IpcEndpoints).as_ref_unchecked() };
    let ipc_queue = endpoints.get(endpoint_name).ok_or(IpcError::EndpointNotFound)?;
    ipc_queue.receive()
}

pub fn receive_answer() -> Result<impl IpcMessageWrapper + use<>, IpcError> {
    let endpoint_name = format!("callback-{}", process::current().id());
    receive(&endpoint_name)
}

pub struct IpcRcvBlockingSyscallParams {
    pub endpoint_name_ptr: *const u8,
    pub service_name_len: usize,
    pub timeout_ms: usize,
}

/// block and wait for a message \
/// will stop waiting after timeout ms
pub fn receive_blocking(endpoint_name: &str, timeout_ms: usize) -> Result<impl IpcMessageWrapper + use<>, IpcError> {
    let endpoints = unsafe { (USER_SPACE_MAILBOX_START as *const IpcEndpoints).as_ref_unchecked() };
    let ipc_queue = endpoints.get(endpoint_name).ok_or(IpcError::EndpointNotFound)?;
    ipc_queue.receive_blocking(timeout_ms)
}

pub fn receive_busy_polling(endpoint_name: &str, timeout_ms: usize, interval_ms: usize) -> Result<impl IpcMessageWrapper + use<>, IpcError> {
    let time_now = syscall::syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap();
    let time_timeout = time_now + timeout_ms;
    loop {
        let rec = receive(endpoint_name);
        if rec.is_ok() {
            return rec;
        }
        if let Err(err) = rec {
            match err {
                IpcError::Empty => {} // keep spinning if empty
                _ => return Err(err), // abort if there is a different error
            }
        }
        if syscall::syscall(syscall::SystemCall::GetSystemTime, &[]).unwrap() >= time_timeout {
            return Err(IpcError::Timeout);
        }
        if interval_ms > 0 {
            thread::sleep(interval_ms);
        }
    }
}

/// Reinterprets an arbitrary struct T into bytes.
///
/// # Safety
///
/// Uses `core::slice::from_raw_parts internally`. Check its docs for the safety requirements.
pub unsafe fn any_as_bytes<T>(any: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts((any as *const T) as *const u8, size_of::<T>()) }
}

/// Reinterprets a byte slice into an arbitrary struct T.
///
/// # Safety
///
/// Manually dereferences the byte slice into the struct T. Will most likely only work correctly when used on a byte slice created by `any_as_bytes`.
pub unsafe fn bytes_as_any<T>(bytes: &[u8]) -> &T {
    unsafe { &*(bytes.as_ptr() as *const T) }
}