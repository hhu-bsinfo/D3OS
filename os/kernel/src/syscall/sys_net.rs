use core::mem::ManuallyDrop;

use alloc::sync::Arc;
use syscall::return_vals::{Errno, SyscallResult};
use smoltcp::wire::Ipv4Address;

use crate::network::{SocketS, bind_udp, close_socket, connect_socket, open_socket};
use crate::naming::{get_open_table_entry};
use net::SocketType;

pub fn sys_socket(protocol: SocketType) -> SyscallResult {
    let (_, fh) = open_socket(protocol)?;

    Ok(fh)
}

pub fn sys_socket_connect(fh: usize, destination_as_u32: u32, port: u16) -> SyscallResult {
    let destination = Ipv4Address::from(destination_as_u32);

    let open_obj = get_open_table_entry(fh)?;

    let f = open_obj.inner_node().as_pseudo()?;

    let socket_struct= unsafe { Arc::from_raw(f.private_data.cast::<SocketS>()) };

    let x = ManuallyDrop::new(socket_struct);

    if !connect_socket(x.handle, destination, port){
        return Err(Errno::EINVAL);
    }

    Ok(0)
}

pub fn sys_socket_bind(fh: usize, port: u16) -> SyscallResult {
    let open_obj = get_open_table_entry(fh)?;

    let f = open_obj.inner_node().as_pseudo()?;

    let socket_struct= unsafe { Arc::from_raw(f.private_data.cast::<SocketS>()) };

    let x = ManuallyDrop::new(socket_struct);

    let _ = bind_udp(x.handle, port).map_err(|_| Errno::EINVAL)?;

    Ok(0)
}

pub fn sys_socket_close(fh: usize) -> SyscallResult {
    let open_obj = get_open_table_entry(fh)?;

    let f = open_obj.inner_node().as_pseudo()?;

    let socket_struct= unsafe { Arc::from_raw(f.private_data.cast::<SocketS>()) };

    close_socket(socket_struct.handle, fh);

    Ok(0)
}