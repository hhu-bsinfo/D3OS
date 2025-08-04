//! This library provides access to the internet -- or other networks.

#![no_std]
extern crate alloc;

use core::{ffi::CStr, net::{IpAddr, Ipv6Addr, SocketAddr}, str::FromStr};

use alloc::{ffi::CString, format, string::ToString, vec::Vec, vec};
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
        // valid addresses do not contain 0 bytes
        let addr = CString::new(address.ip().to_string()).unwrap();
        syscall(SystemCall::SockBind, &[
            handle,
            protocol,
            addr.as_bytes_with_nul().as_ptr() as usize,
            address.port().into(),
        ])
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
    
    pub fn recv_from(&self, data_buf: &mut [u8]) -> Result<(usize, SocketAddr), NetworkError> {
        let protocol = 0;
        // this should be the maximum length for an IP address
        let mut addr_buf = [0u8; 40];
        let result = syscall(SystemCall::SockReceive, &[
            self.handle,
            protocol,
            data_buf.as_ptr() as usize,
            data_buf.len(),
            addr_buf.as_mut_ptr() as usize,
        ])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        // This just exists for UDP. TCP and ICMP get the full isize for len.
        let num_bytes = result >> 16;
        let remote_port = result as u16;
        let remote_addr = if num_bytes > 0 {
            let addr_str = CStr::from_bytes_until_nul(&addr_buf).unwrap().to_str().unwrap();
            SocketAddr::new(
                IpAddr::from_str(&addr_str).expect(&format!("failed to parse '{addr_str}'")),
                remote_port,
            )
        } else {
            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
        };
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
        // valid addresses do not contain 0 bytes
        let addr = CString::new(address.ip().to_string()).unwrap();
        syscall(SystemCall::SockBind, &[
            handle,
            protocol,
            addr.as_bytes_with_nul().as_ptr() as usize,
            address.port().into(),
        ])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?;
        Ok(Self { handle, address })
    }

    pub fn accept(&self) -> Result<TcpStream, NetworkError> {
        let protocol = 1;
        // this should be the maximum length for an IP address
        let mut addr_buf = [0u8; 40];
        let remote_port: u16 = syscall(SystemCall::SockAccept, &[
            self.handle,
            protocol,
            addr_buf.as_mut_ptr() as usize,
        ])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?
            .try_into().unwrap();
        let addr_str = CStr::from_bytes_until_nul(&addr_buf).unwrap().to_str().unwrap();
        let remote_addr = SocketAddr::new(
            IpAddr::from_str(&addr_str).expect(&format!("failed to parse '{addr_str}'")),
            remote_port,
        );
        Ok(TcpStream { handle: self.handle, local_address: self.address, peer_address: remote_addr })
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
        // this should be the maximum length for an IP address
        let mut addr_buf = [0u8; 40];
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
            addr_buf.as_mut_ptr() as usize,
        ])
            .map_err(|errno| match errno {
                Errno::EEXIST => panic!("socket as already been opened"),
                Errno::EINVAL => NetworkError::InvalidAddress,
                errno => NetworkError::Unknown(errno),
            })?
            .try_into().unwrap();
        let addr_str = CStr::from_bytes_until_nul(&addr_buf).unwrap().to_str().unwrap();
        let local_address = SocketAddr::new(
            IpAddr::from_str(&addr_str).expect(&format!("failed to parse '{addr_str}'")),
            local_port,
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
        // ICMP doesn't bind to an IP address, but the syscall still expects one.
        let addr = CString::new(Ipv6Addr::UNSPECIFIED.to_string()).unwrap();
        syscall(SystemCall::SockBind, &[
            handle,
            protocol,
            addr.as_bytes_with_nul().as_ptr() as usize,
            ident.into(),
        ])
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

     pub fn recv(&self, buf: &mut [u8]) -> Result<(usize, IpAddr), NetworkError> {
        let protocol = 2;
        // this should be the maximum length for an IP address
        let mut addr_buf = [0u8; 40];
        let num_bytes = syscall(SystemCall::SockReceive, &[
            self.handle,
            protocol,
            buf.as_ptr() as usize,
            buf.len(),
            addr_buf.as_mut_ptr() as usize,
        ])
            .map_err(|errno| match errno {
                Errno::ENOTSUP => panic!("invalid protocol"),
                errno => NetworkError::Unknown(errno),
            })?;
        let address = if num_bytes > 0 {
            let addr_str = CStr::from_bytes_until_nul(&addr_buf).unwrap().to_str().unwrap();
            IpAddr::from_str(&addr_str).expect(&format!("failed to parse '{addr_str}'"))
        } else {
            IpAddr::V6(Ipv6Addr::UNSPECIFIED)
        };

        Ok((num_bytes, address))
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

/// Get all IP addresses of this host.
pub fn get_ip_addresses() -> Vec<IpAddr> {
    let mut buf = [0u8; 4096];
    // this can't fail, unless the buffer is not big enough
    syscall(SystemCall::GetIpAddresses, &[
        buf.as_mut_ptr() as usize,
        buf.len(),
    ]).unwrap();
    split_ips(&buf)
}

/// Resolve this hostname, return a list of IP addresses.
pub fn resolve_hostname(host: &str) -> Vec<IpAddr> {
    // this might already be an IP address
    if let Ok(ip) = host.parse() {
        vec![ip]
    } else {
        let host_c = CString::new(host).unwrap();
        let mut buf = [0u8; 4096];
        // this can't fail, unless the buffer is not big enough
        syscall(SystemCall::GetIpAddresses, &[
            buf.as_mut_ptr() as usize,
            buf.len(),
            host_c.as_bytes_with_nul().as_ptr() as usize,
        ]).unwrap();
        split_ips(&buf)
    }
}


/// Split a \0-byte seperated list of IP addresses
fn split_ips(buf: &[u8]) -> Vec<IpAddr> {
    buf
        // split them at \0
        .split( |v| v == &0)
        // ignore the empty ones at the end
        .filter(|v| !v.is_empty())
        // parse them as strings
        .map(str::from_utf8)
        .map(Result::unwrap)
        // parse them as IP addresses
        .map(str::parse)
        .map(Result::unwrap)
        // return
        .collect()
}
