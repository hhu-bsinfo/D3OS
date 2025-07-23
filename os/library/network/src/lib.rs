//! This library provides access to the internet -- or other networks.

#![no_std]
extern crate alloc;

use core::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

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
            buf.as_ptr() as usize,
            buf.len(),
            addr.as_bytes_with_nul().as_ptr() as usize,
            address.port().into(),
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

pub struct TcpListener {
    handle: usize,
    /// the (local) address this socket is bound to
    address: SocketAddr
}

impl TcpListener {
    pub fn bind(address: SocketAddr) -> Result<Self, NetworkError> {
        let protocol = 1;
        let handle = syscall(SystemCall::SockOpen, &[protocol])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        // TODO: also pass the address
        syscall(SystemCall::SockBind, &[handle, protocol, address.port().into()])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?;
        Ok(Self { handle, address })
    }

    pub fn accept(&self) -> Result<TcpStream, NetworkError> {
        let protocol = 1;
        let peer_port: u16 = syscall(SystemCall::SockAccept, &[self.handle, protocol])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?
            .try_into().unwrap();
        // TODO: also pass the remote SocketAddr
        let peer_address = SocketAddr::V4(
            SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), peer_port)
        );
        Ok(TcpStream { handle: self.handle, local_address: self.address, peer_address })
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        // TODO: just drop this when all connections are gone
    }
}


pub struct TcpStream {
    handle: usize,
    local_address: SocketAddr,
    peer_address: SocketAddr,
}

impl TcpStream {
    pub fn connect(address: SocketAddr) -> Result<Self, NetworkError> {
        let protocol = 1;
        // valid addresses do not contain 0 bytes
        let addr = CString::new(address.ip().to_string()).unwrap();
        let handle = syscall(SystemCall::SockOpen, &[protocol])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        let local_port: u16 = syscall(SystemCall::SockConnect, &[
            handle,
            protocol,
            addr.as_bytes_with_nul().as_ptr() as usize,
            address.port().into(),
        ])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?
            .try_into().unwrap();
        let local_address = SocketAddr::V4(
            SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), local_port)
        );
        Ok(Self { handle, local_address, peer_address: address })
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, NetworkError> {
        let protocol = 1;
        syscall(SystemCall::SockSend, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
        ])
            .map_err(|errno| match errno {
                Errno::EINVAL => panic!("socket can't send"),
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, NetworkError> {
        let protocol = 1;
        let num_bytes = syscall(SystemCall::SockReceive, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
        ])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;

        Ok(num_bytes)
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let protocol = 1;
        syscall(SystemCall::SockClose, &[self.handle, protocol])
            .expect("failed to close socket");
    }
}

pub struct IcmpSocket {
    handle: usize,
    ident: u16,
}

impl IcmpSocket {
    pub fn bind(ident: u16) -> Result<Self, NetworkError> {
        let protocol = 2;
        let handle = syscall(SystemCall::SockOpen, &[protocol])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        syscall(SystemCall::SockBind, &[handle, protocol, ident.into()])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket has already been openend"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno)
            })?;
        Ok(Self { handle, ident })
    }

    pub fn send_to(&self, buf: &[u8], address: IpAddr) -> Result<usize, NetworkError> {
        let protocol = 2;
        // valid addresses do not contain 0 bytes
        let addr = CString::new(address.to_string()).unwrap();
        syscall(SystemCall::SockSend, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
            addr.as_bytes_with_nul().as_ptr() as usize,
        ])
            .map_err(|errno| match errno {
                Errno::EINVAL => NetworkError::InvalidAddress,
                Errno::EBUSY => NetworkError::DeviceBusy,
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })
    }

     pub fn recv(&self, buf: &mut [u8]) -> Result<usize, NetworkError> {
        let protocol = 2;
        let num_bytes = syscall(SystemCall::SockReceive, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
        ])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;

        Ok(num_bytes)
    }
}

impl Drop for IcmpSocket {
    fn drop(&mut self) {
        let protocol = 2;
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
