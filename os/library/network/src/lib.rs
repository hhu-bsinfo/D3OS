//! This library provides access to the internet -- or other networks.

#![no_std]
extern crate alloc;

use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use alloc::{ffi::CString, string::ToString};
use syscall::{return_vals::Errno, syscall, SystemCall};

pub struct UdpSocket {
    handle: usize,
    /// the (local) address this socket is bound to
    address: SocketAddr,
}

impl UdpSocket {
    pub fn bind(address: SocketAddr) -> Result<Self, NetworkError> {
        let protocol = 0;
        let handle = syscall(SystemCall::SockOpen, &[protocol])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        // TODO: also pass the address
        syscall(SystemCall::SockBind, &[handle, protocol, address.port().into()])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket has already been openend"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno)
            })?;
        Ok(Self { handle, address })
    }

    pub fn send_to(&self, buf: &[u8], address: SocketAddr) -> Result<usize, NetworkError> {
        let protocol = 0;
        // valid addresses do not contain 0 bytes
        let addr = CString::new(address.ip().to_string()).unwrap();
        syscall(SystemCall::SockSend, &[
            self.handle,
            protocol,
            addr.as_bytes_with_nul().as_ptr() as usize,
            address.port().into(),
            buf.as_ptr() as usize,
            buf.len(),
        ])
            .map_err(|errno| match errno {
                Errno::EINVAL => NetworkError::InvalidAddress,
                Errno::EBUSY => NetworkError::DeviceBusy,
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })
    }
    
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetworkError> {
        let protocol = 0;
        let num_bytes = syscall(SystemCall::SockReceive, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
            // TODO: also get IP addr and port
        ])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        let remote_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0));

        Ok((num_bytes, remote_addr))
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        let protocol = 0;
        syscall(SystemCall::SockClose, &[self.handle, protocol])
            .expect("failed to close socket");
    }
}

#[derive(Debug)]
pub enum NetworkError {
    DeviceBusy,
    InvalidAddress,
    Unknown(Errno),
}
