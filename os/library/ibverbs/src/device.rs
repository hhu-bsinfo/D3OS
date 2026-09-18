use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use bincode::{Decode, Encode};
use core3::io;
use rdma::{DeviceHandle, PortAttr};
use crate::context::Context;
use crate::PORT_NUM;
use crate::provider::{get_available_devices, open_device};

/// Get list of available RDMA devices.
///
/// # Errors
///
///  - `EPERM`: Permission denied.
///  - `ENOMEM`: Insufficient memory to complete the operation.
///  - `ENOSYS`: No kernel support for RDMA.
pub fn devices() -> io::Result<DeviceList> {
    Ok(DeviceList(get_available_devices()?))
}

/// List of available RDMA devices.
pub struct DeviceList(Vec<DeviceHandle>);

unsafe impl Sync for DeviceList {}
unsafe impl Send for DeviceList {}

impl DeviceList {
    /// Returns an iterator over all found devices.
    pub fn iter(&self) -> DeviceListIter<'_> {
        DeviceListIter { list: self, i: 0 }
    }

    /// Returns the number of devices.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are any devices.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the device at the given `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<Device> {
        self.0.get(index).map(|h| Device::new(*h))
    }
}

impl<'a> IntoIterator for &'a DeviceList {
    type Item = <DeviceListIter<'a> as Iterator>::Item;
    type IntoIter = DeviceListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        DeviceListIter { list: self, i: 0 }
    }
}

/// Iterator over a `DeviceList`.
pub struct DeviceListIter<'iter> {
    list: &'iter DeviceList,
    i: usize,
}

impl Iterator for DeviceListIter<'_> {
    type Item = Device;
    fn next(&mut self) -> Option<Self::Item> {
        let e = self.list.0.get(self.i);
        if e.is_some() {
            self.i += 1;
        }
        e.map(|h| Device::new(*h))
    }
}

/// An RDMA device.
pub struct Device {
    pub handle: DeviceHandle,
}

impl Device {
    fn new(handle: DeviceHandle) -> Self {
        Device { handle }
    }

    pub fn open(&self) -> io::Result<Context> {
        Context::with_device(self)
    }

    pub fn name(&self) -> Option<String> {
        Some("mlx4_todo".to_string())
    }

    /// Query this device's port without opening a full context.
    ///
    /// [`Self::open`] refuses any port that is not `ACTIVE` or `ARMED`, which is right for
    /// anything that intends to transfer data but useless for diagnostics: it means the port
    /// cannot be inspected exactly when something has gone wrong with it. This skips both that
    /// check and the GID query (whose result is only defined for an active port).
    pub fn port_attr(&self) -> io::Result<PortAttr> {
        open_device(&self)?.query_port(PORT_NUM)
    }
}

/// A Global unique identifier for ibv.
///
/// This struct acts as a rust wrapper for GUID value represented as `__be64` in
/// libibverbs. We introduce this struct, because u64 is stored in host
/// endianness, whereas ibverbs stores GUID in network order (big endian).
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Guid {
    raw: [u8; 8],
}

impl Guid {
    /// Upper 24 bits of the GUID are OUI (Organizationally Unique Identifier,
    /// http://standards-oui.ieee.org/oui/oui.txt). The function returns OUI as
    /// a 24-bit number inside a u32.
    pub fn oui(&self) -> u32 {
        let padded = [0, self.raw[0], self.raw[1], self.raw[2]];
        u32::from_be_bytes(padded)
    }

    /// Returns `true` if this GUID is all zeroes, which is considered reserved.
    pub fn is_reserved(&self) -> bool {
        self.raw == [0; 8]
    }
}

impl From<u64> for Guid {
    fn from(guid: u64) -> Self {
        Self {
            raw: guid.to_be_bytes(),
        }
    }
}

impl From<Guid> for u64 {
    fn from(guid: Guid) -> Self {
        u64::from_be_bytes(guid.raw)
    }
}

