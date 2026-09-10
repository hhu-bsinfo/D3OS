#![no_std]

pub mod ipc_message;
pub mod ipc_queue;
pub mod const_string;

extern crate alloc;

use alloc::format;
use syscall::{SystemCall, return_vals::SyscallResult, syscall};

use concurrent::process;

use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageWrapper};
use crate::ipc_queue::IpcQueue;

// Schnittstelle für Interprozesskommunikation

pub struct IpcSyscallParams {
    pub service_name_ptr: *const u8,
    pub service_name_len: usize,
    pub msg_name_ptr: *const u8,
    pub msg_name_len: usize,
    pub msg_type_ptr: *const u8,
    pub msg_type_len: usize,
    pub data_ptr: *const u8,
    pub data_len: usize,
}

const MESSAGE_NAME_LEN: usize = 0x20;
const MESSAGE_TAG_LEN: usize = 0x10;
//pub const MESSAGE_DATA_SIZE: usize = 0x100 - (size_of::<Uuid>() + size_of::<usize>() + MESSAGE_TAG_LEN);
pub const MESSAGE_DATA_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 32; // !this needs to be a power of 2! also this can not be smaller than 4

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

pub fn register_ipc_endpoint(svc_name: &str) -> Result<usize, syscall::return_vals::Errno> {
    // TODO what happens when we register multiple endpoints / multiple services for this endpoint
    let mailbox_start = USER_SPACE_MAILBOX_START as *const u8;
    let ipc_queue_ptr = mailbox_start as *mut IpcQueue;
    unsafe { 
        ipc_queue_ptr.write(IpcQueue::new());
    }
    let name_ptr = svc_name.as_ptr();
    let name_len = svc_name.len();
    syscall(SystemCall::IpcRegister, &[name_ptr as usize, name_len])
}

pub fn remove_ipc_endpoint(svc_name: &str) -> Result<usize, syscall::return_vals::Errno> {
    let name_ptr = svc_name.as_ptr();
    let name_len = svc_name.len();
    syscall(SystemCall::IpcDelete, &[name_ptr as usize, name_len])
}

pub fn send<T: IpcMessageContent>(service: &str, msg_name: &str, message: T) -> SyscallResult {
    let bytes: &[u8] = message.to_bytes();
    send_internal(service, msg_name, message.tag(), bytes)
}

pub fn send_with_tag<T: IpcMessageContent>(service: &str, msg_name: &str, tag: &str, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(service, msg_name, tag, bytes)
}

pub fn send_struct<T: Copy>(service: &str, msg_name: &str, message: &T) -> SyscallResult {
    let bytes = unsafe { any_as_bytes(message) };
    let tag = core::any::type_name_of_val(message);
    send_internal(service, msg_name, tag, bytes)
}

pub fn answer<T: IpcMessageContent>(original_message: &IpcMessage, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(&format!("callback-{}", original_message.sender_pid), &format!("answer-{}", original_message.name), message.tag(), bytes)
}

pub fn answer_with_name<T: IpcMessageContent>(original_message: &IpcMessage, msg_name: &str, message: T) -> SyscallResult {
    let bytes = message.to_bytes();
    send_internal(&format!("callback-{}", original_message.sender_pid), msg_name, message.tag(), bytes)
}

pub fn send_internal(service: &str, msg_name: &str, tag: &str, message: &[u8]) -> SyscallResult {

    // register callback endpoint
    let pid = process::current().id();
    let _res = register_ipc_endpoint(&format!("callback-{}", pid));

    let params = IpcSyscallParams {
        service_name_ptr: service.as_ptr(),
        service_name_len: service.len(),
        msg_name_ptr: msg_name.as_ptr(),
        msg_name_len: msg_name.len(),
        msg_type_ptr: tag.as_ptr(),
        msg_type_len: tag.len(),
        data_ptr: message.as_ptr(),
        data_len: message.len(),
    };

    syscall(SystemCall::IpcSend, &&params)
}

pub fn receive() -> Option<IpcMessageWrapper> {
    let ipc_queue = unsafe { (USER_SPACE_MAILBOX_START as *const IpcQueue).as_ref() };
    ipc_queue.and_then(|iq| iq.receive())
}

pub fn receive_blocking(timeout_ms: usize) -> Result<IpcMessageWrapper, syscall::return_vals::Errno> {
    match syscall(SystemCall::IpcReceiveBlocking, &[timeout_ms]) {
        Ok(_) => receive().ok_or(syscall::return_vals::Errno::EEXIST),
        Err(e) => Err(e),
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
/// Manually dereferences the byte slice into the struct T. Will probably only work correctly when uses on a byte slice created by `any_as_bytes`.
pub unsafe fn bytes_as_any<T>(bytes: &[u8]) -> &T {
    unsafe { &*(bytes.as_ptr() as *const T) }
}