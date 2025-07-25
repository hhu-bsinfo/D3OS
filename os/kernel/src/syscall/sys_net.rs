use core::str::FromStr;

use alloc::string::ToString;
use log::{info, warn};
use smoltcp::{iface::SocketHandle, socket::{icmp, tcp, udp}, wire::IpAddress};
use syscall::return_vals::Errno;

use crate::{network::{accept_tcp, bind_icmp, bind_tcp, bind_udp, close_socket, connect_tcp, get_ip_addresses, open_icmp, open_tcp, open_udp, receive_datagram, receive_icmp, receive_tcp, send_datagram, send_icmp, send_tcp, SocketType}, syscall::sys_naming::ptr_to_string};

/// This module contains all network-related system calls.

pub fn sys_sock_open(protocol: SocketType) -> isize {
    info!("opening a {protocol:?} socket");
    // TODO: what happens when we get a type thats not in the enum?
    // TODO: can we somehow bind this socket to the process,
    // so that we know which process has opened this socket
    // and are able to close it on process exit
    #[allow(unreachable_patterns)]
    let handle = match protocol {
        SocketType::Udp => open_udp(),
        SocketType::Tcp => open_tcp(),
        SocketType::Icmp => open_icmp(),
        _ => return Errno::ENOTSUP.into(),
    };
    // handle.0 is private, sadly, so just hope this works
    unsafe { core::mem::transmute::<SocketHandle, usize>(handle) }.try_into().unwrap()
}

pub fn sys_sock_bind(handle: SocketHandle, protocol: SocketType, port: u16) -> isize  {
    // TODO: somehow check that the protocol is correct for handle?
    // TODO: allow binding to anything other than ::
    info!("binding {handle:?} to {port}");
    #[allow(unreachable_patterns)]
    match protocol {
        SocketType::Udp => match bind_udp(handle, port) {
            Ok(()) => 0,
            // socket has already been opened
            Err(udp::BindError::InvalidState) => Errno::EEXIST.into(),
            // port is zero
            Err(udp::BindError::Unaddressable) => Errno::EINVAL.into(),
        },
        SocketType::Tcp => match bind_tcp(handle, port) {
            Ok(()) => 0,
            // socket has already been opened
            Err(tcp::ListenError::InvalidState) => Errno::EEXIST.into(),
            // port is zero
            Err(tcp::ListenError::Unaddressable) => Errno::EINVAL.into(),
        },
        // port is actually the ident here
        SocketType::Icmp => match bind_icmp(handle, port) {
            Ok(()) => 0,
            // socket has already been opened
            Err(icmp::BindError::InvalidState) => Errno::EEXIST.into(),
            // ident is missing
            Err(icmp::BindError::Unaddressable) => Errno::EINVAL.into(),
        }
        _ => Errno::ENOTSUP.into(),
    }
}

pub unsafe fn sys_sock_accept(
    handle: SocketHandle,
    protocol: SocketType,
) -> isize {
    if matches!(protocol, SocketType::Tcp) {
        match accept_tcp(handle) {
            Ok(port) => port.try_into().unwrap(),
            Err(e) => panic!("failed to accept: {e:?}"),
        }
    } else {
        Errno::ENOTSUP.into()
    }
}

pub unsafe fn sys_sock_connect(
    handle: SocketHandle,
    protocol: SocketType,
    addr_ptr: *const u8,
    port: u16,
) -> isize {
    if matches!(protocol, SocketType::Tcp) {
        if let Ok(addr_str) = unsafe { ptr_to_string(addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
            match connect_tcp(handle, addr, port) {
                Ok(port) => port.try_into().unwrap(),
                Err(e) => panic!("failed to accept: {e:?}"),
            }
        } else {
            Errno::EINVAL.into()
        }
    } else {
        Errno::ENOTSUP.into()
    }
}

pub unsafe fn sys_sock_send(
    handle: SocketHandle,
    protocol: SocketType,
    data: *const u8,
    len: usize,
    addr_ptr: *const u8,
    port: u16,
) -> isize {
    let data = unsafe { core::slice::from_raw_parts(data, len) };
    #[allow(unreachable_patterns)]
    match protocol {
        SocketType::Udp => {
            if let Ok(addr_str) = unsafe { ptr_to_string(addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
                match send_datagram(handle, addr, port, data) {
                    Ok(()) => data.len().try_into().unwrap(),
                    // host or port are missing or zero
                    Err(udp::SendError::Unaddressable) => Errno::EINVAL.into(),
                    // TODO: drop? return 0?
                    Err(udp::SendError::BufferFull) => Errno::EBUSY.into(),
                }
            } else {
                Errno::EINVAL.into()
            }
        },
        SocketType::Tcp => match send_tcp(handle, data) {
            Ok(len) => len.try_into().unwrap(),
            // socket can't send (yet)
            Err(tcp::SendError::InvalidState) => Errno::EINVAL.into(),
        },
        SocketType::Icmp => {
            if let Ok(addr_str) = unsafe { ptr_to_string(addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
                match send_icmp(handle, addr, data) {
                    Ok(()) => 0,
                    // ip address missing
                    Err(icmp::SendError::Unaddressable) => Errno::EINVAL.into(),
                    // TODO: drop? return 0?
                    Err(icmp::SendError::BufferFull) => Errno::EBUSY.into(),
                }
            } else {
                Errno::EINVAL.into()
            }
        }
        _ => Errno::ENOTSUP.into(),
    }
}

pub unsafe fn sys_sock_receive(
    handle: SocketHandle,
    protocol: SocketType,
    data_ptr: *mut u8,
    data_len: usize,
) -> isize {
    let data = unsafe { core::slice::from_raw_parts_mut(data_ptr, data_len) };
    #[allow(unreachable_patterns)]
    match protocol {
        SocketType::Udp => match receive_datagram(handle, data) {
            // TODO: also pass the metadata
            Ok((len, metadata)) => len.try_into().unwrap(),
            // discard truncated packet
            Err(udp::RecvError::Truncated) => {
                warn!("discarding truncated incoming packet");
                0
            },
            // if we got no data, that is okay
            Err(udp::RecvError::Exhausted) => 0,
        },
        SocketType::Tcp => match receive_tcp(handle, data) {
            Ok(len) => len.try_into().unwrap(),
            Err(tcp::RecvError::InvalidState) => {
                warn!("TCP socket is in an invalid state");
                Errno::EINVALH.into()
            },
            // the remote host closed the connection
            Err(tcp::RecvError::Finished) => Errno::ECONNRESET.into(),
        },
        SocketType::Icmp => match receive_icmp(handle, data) {
            // TODO: also pass the address
            Ok((len, address)) => len.try_into().unwrap(),
            // discard truncated packet
            Err(icmp::RecvError::Truncated) => {
                warn!("discarding truncated incoming packet");
                0
            },
            // if we got no data, that is okay
            Err(icmp::RecvError::Exhausted) => 0,
        },
        _ => Errno::ENOTSUP.into(),
    }
}

pub fn sys_sock_close(handle: SocketHandle) -> isize {
    info!("closing {handle} socket");
    close_socket(handle);
    0
}

/// return a \0 seperated list of ip addresses
pub fn sys_get_ip_adresses(ptr: *mut u8, len: usize) -> isize {
    let target = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
    let mut idx = 0;
    for ip in get_ip_addresses() {
        let text = ip.to_string();
        target[idx..idx+text.len()].copy_from_slice(text.as_bytes());
        target[idx+text.len()] = 0;
        idx += text.len() + 1;
    }
    0
}
