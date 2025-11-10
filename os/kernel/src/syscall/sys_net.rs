use core::str::FromStr;

use alloc::{ffi::CString, string::ToString};
use log::{debug, info, warn};
use smoltcp::{iface::SocketHandle, socket::{icmp, tcp, udp}, wire::IpAddress};
use syscall::return_vals::Errno;

use crate::{naming::virtual_objects::recover_pseudo, network::{accept_tcp, bind_icmp, bind_tcp, bind_udp, close_socket, connect_tcp, get_ip_addresses, open_icmp, open_tcp, open_udp, receive_datagram, receive_icmp, receive_tcp, send_datagram, send_icmp, send_tcp}, syscall::sys_naming::ptr_to_string};

/// This module contains all network-related system calls.

pub fn sys_sock_open(protocol: SocketType) -> isize {
    info!("opening a {protocol:?} socket");
    // TODO: what happens when we get a type thats not in the enum?
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

pub unsafe fn sys_sock_bind(
    handle: SocketHandle, protocol: SocketType, addr_ptr: *const u8, port: u16,
) -> isize {
    // TODO: somehow check that the protocol is correct for handle?
    if let Ok(addr_str) = unsafe { ptr_to_string(addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
        info!("binding {handle:?} to {addr:?}:{port}");
        #[allow(unreachable_patterns)]
        match protocol {
            SocketType::Udp => match bind_udp(handle, addr, port) {
                Ok(()) => 0,
                // socket has already been opened
                Err(udp::BindError::InvalidState) => Errno::EEXIST.into(),
                // port is zero
                Err(udp::BindError::Unaddressable) => Errno::EINVAL.into(),
            },
            SocketType::Tcp => match bind_tcp(handle, addr, port) {
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
    } else {
        Errno::EINVAL.into()
    }
}

pub unsafe fn sys_sock_accept(
    handle: SocketHandle,
    protocol: SocketType,
    addr_buf: *mut u8,
) -> isize {
    if matches!(protocol, SocketType::Tcp) {
        info!("accepting connections on {handle:?}");
        match accept_tcp(handle) {
            Ok(endpoint) => {
                let addr_str = CString::new(
                    endpoint.addr.to_string().as_bytes()
                ).unwrap();
                let addr_bytes = addr_str.as_bytes_with_nul();
                unsafe { addr_buf.copy_from_nonoverlapping(
                    addr_bytes.as_ptr(), addr_bytes.len(),
                ) };
                endpoint.port.try_into().unwrap()
            },
            Err(e) => panic!("failed to accept: {e:?}"),
        }
    } else {
        Errno::ENOTSUP.into()
    }
}

pub unsafe fn sys_sock_connect(
    handle: SocketHandle,
    protocol: SocketType,
    remote_addr_ptr: *const u8,
    port: u16,
    local_addr_ptr: *mut u8,
) -> isize {
    if matches!(protocol, SocketType::Tcp) {
        if let Ok(addr_str) = unsafe { ptr_to_string(remote_addr_ptr) } && let Ok(addr) = IpAddress::from_str(&addr_str) {
            info!("connecting to {addr:?}:{port}");
            match connect_tcp(handle, addr, port) {
                Ok(endpoint) => {
                    let addr_str = CString::new(
                        endpoint.addr.to_string().as_bytes()
                    ).unwrap();
                    let addr_bytes = addr_str.as_bytes_with_nul();
                    unsafe { local_addr_ptr.copy_from_nonoverlapping(
                        addr_bytes.as_ptr(), addr_bytes.len(),
                    ) };
                    endpoint.port.try_into().unwrap()
                },
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
    debug!("sending {len} bytes on {handle:?}");
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
    addr_buf: *mut u8,
) -> isize {
    let data = unsafe { core::slice::from_raw_parts_mut(data_ptr, data_len) };
    debug!("receiving up to {data_len} bytes on {handle:?}");
    #[allow(unreachable_patterns)]
    match protocol {
        SocketType::Udp => match receive_datagram(handle, data) {
            // TODO: also pass the metadata
            Ok((len, metadata)) => {
                let addr_str = CString::new(
                    metadata.endpoint.addr.to_string().as_bytes()
                ).unwrap();
                let addr_bytes = addr_str.as_bytes_with_nul();
                unsafe { addr_buf.copy_from_nonoverlapping(
                    addr_bytes.as_ptr(), addr_bytes.len(),
                ) };
                let mut val = isize::try_from(len << 16).unwrap();
                val |= isize::try_from(metadata.endpoint.port).unwrap();
                val
            },
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
            Ok((len, address)) => {
                let addr_str = CString::new(
                    address.to_string().as_bytes()
                ).unwrap();
                let addr_bytes = addr_str.as_bytes_with_nul();
                unsafe { addr_buf.copy_from_nonoverlapping(
                    addr_bytes.as_ptr(), addr_bytes.len(),
                ) };
                len.try_into().unwrap()
            },
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

/// Return a \0 seperated list of IP addresses for a given hostname.
/// 
/// If the hostname is missing, the addresses of the current host will be returned.
pub unsafe fn sys_get_ip_adresses(ptr: *mut u8, len: usize, host_ptr: *const u8) -> isize {
    let host = if host_ptr.is_null() {
        None
    } else {
        match unsafe { ptr_to_string(host_ptr) } {
            Ok(host) => Some(host),
            Err(errno) => return errno.into(),
        }
    };
    info!("resolving host {host:?}");
    let target = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
    let mut idx = 0;
    for ip in get_ip_addresses(host.as_deref()) {
        info!("{host:?} has address {ip:?}");
        let text = ip.to_string();
        target[idx..idx+text.len()].copy_from_slice(text.as_bytes());
        target[idx+text.len()] = 0;
        idx += text.len() + 1;
    }
    0
}

/*********************************

Naming service based approach

**********************************/

use core::mem::ManuallyDrop;

use syscall::return_vals::{SyscallResult};
use smoltcp::wire::Ipv4Address;

use crate::network::{SocketS, close_socket_legacy, connect_socket, open_socket};
use net::SocketType;

pub fn sys_socket(protocol: SocketType) -> SyscallResult {
    let (_, fh) = open_socket(protocol)?;

    Ok(fh)
}

pub fn sys_socket_connect(fh: usize, destination_as_u32: u32, port: u16) -> SyscallResult {
    let destination = Ipv4Address::from(destination_as_u32);

    let socket_struct = recover_pseudo::<SocketS>(fh)?;

    let x = ManuallyDrop::new(socket_struct);

    if !connect_socket(x.handle, destination, port){
        return Err(Errno::EINVAL);
    }

    Ok(0)
}

pub fn sys_socket_bind(fh: usize, port: u16) -> SyscallResult {
    let socket_struct = recover_pseudo::<SocketS>(fh)?;

    let x = ManuallyDrop::new(socket_struct);

    let _ = bind_udp(x.handle, IpAddress::Ipv4(Ipv4Address::UNSPECIFIED), port).map_err(|_| Errno::EINVAL)?;

    Ok(0)
}

pub fn sys_socket_close(fh: usize) -> SyscallResult {
    let socket_struct = recover_pseudo::<SocketS>(fh)?;

    close_socket_legacy(socket_struct.handle, fh);

    Ok(0)
}
