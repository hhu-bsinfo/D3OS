use core::str::FromStr;

use log::{info, warn};
use smoltcp::{iface::SocketHandle, socket::udp::{self, BindError}, wire::IpAddress};
use syscall::return_vals::Errno;

use crate::{network::{bind_udp, close_socket, open_socket, receive_datagram, send_datagram, SocketType}, syscall::sys_naming::ptr_to_string};

/// This module contains all network-related system calls.

pub fn sys_sock_open(protocol: SocketType) -> isize {
    info!("opening a {protocol:?} socket");
    // TODO: what happens when we get a type thats not in the enum?
    // TODO: can we somehow bind this socket to the process,
    // so that we know which process has opened this socket
    // and are able to close it on process exit
    if let Some(handle) = open_socket(protocol) {
        // handle.0 is private, sadly, so just hope this works
        unsafe { core::mem::transmute::<SocketHandle, usize>(handle) }.try_into().unwrap()
    } else {
        // unknown protocol
        Errno::ENOTSUP.into()
    }
}

pub fn sys_sock_bind(handle: SocketHandle, protocol: SocketType, port: u16) -> isize  {
    // TODO: somehow check that the protocol is correct for handle?
    // TODO: allow binding to anything other than ::
    info!("binding {handle:?} to {port}");
    match match protocol {
        SocketType::Udp => bind_udp(handle, port),
    } {
        Ok(()) => 0,
        // socket has already been opened
        Err(BindError::InvalidState) => Errno::EEXIST.into(),
        // port is zero
        Err(BindError::Unaddressable) => Errno::EINVAL.into(),
    }
}

pub unsafe fn sys_sock_send(
    handle: SocketHandle,
    protocol: SocketType,
    addr_ptr: *const u8,
    port: u16,
    data: *const u8,
    len: usize,
) -> isize {
    if let Ok(addr_str) = unsafe { ptr_to_string(addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
        let data = unsafe { core::slice::from_raw_parts(data, len) };
        match match protocol {
            SocketType::Udp => send_datagram(handle, addr, port, data),
            _ => return Errno::ENOTSUP.into(),
        } {
            Ok(()) => data.len().try_into().unwrap(),
            // host or port are missing or zero
            Err(udp::SendError::Unaddressable) => Errno::EINVAL.into(),
            // TODO: drop? return 0?
            Err(udp::SendError::BufferFull) => Errno::EBUSY.into(),
        }
    } else {
        Errno::EINVAL.into()
    }
}

pub unsafe fn sys_sock_receive(
    handle: SocketHandle,
    protocol: SocketType,
    data_ptr: *mut u8,
    data_len: usize,
) -> isize {
    let data = unsafe { core::slice::from_raw_parts_mut(data_ptr, data_len) };
    match match protocol {
        SocketType::Udp => receive_datagram(handle, data),
        _ => return Errno::ENOTSUP.into(),
    } {
        // TODO: also pass the metadata
        Ok((len, metadata)) => len.try_into().unwrap(),
        // discard truncated packet
        Err(udp::RecvError::Truncated) => {
            warn!("discarding truncated incoming packet");
            0
        },
        // if we got no data, that is okay
        Err(udp::RecvError::Exhausted) => 0,
    }
}

pub fn sys_sock_close(handle: SocketHandle) -> isize {
    info!("closing {handle} socket");
    close_socket(handle);
    0
}
