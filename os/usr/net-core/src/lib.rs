#![no_std]

use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use net::SocketType;
use smoltcp::wire::Ipv4Address;
use naming;

pub fn socket(protocol: SocketType) -> SyscallResult {
    syscall(SystemCall::SocketOpen, &[
        protocol.into()]
    )
}

pub fn connect(fh: usize, destination: Ipv4Address, port: u16) -> SyscallResult {
    syscall(SystemCall::SocketConnect, &[
        fh,
        u32::from(destination) as usize,
        port.into()
    ])
}

pub fn bind(fh: usize, port: u16) -> SyscallResult {
    syscall(SystemCall::SocketBind, &[
        fh,
        port.into()
    ])
}

pub fn close(fh: usize) -> SyscallResult {
    syscall(SystemCall::SocketClose, &[
        fh
    ])
}

// wrapper around actual naming serv. read
pub fn read(fh: usize, buf: &mut [u8]) -> SyscallResult {
    naming::read(fh, buf)
}

// wrapper around actual naming serv. write
pub fn write(fh: usize, buf: & [u8]) -> SyscallResult {
    naming::write(fh, buf)
}