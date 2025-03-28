use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr::NonNull;
use pci_types::PciAddress;
use spin::Mutex;
use tock_registers::interfaces::Readable;
use tock_registers::register_bitfields;
use tock_registers::registers::ReadWrite;
use x86_64::instructions::port::{Port, PortReadOnly};
use crate::device::pci::ConfigurationSpace;
use crate::device::virtio::transport::flags::DeviceStatusFlags;

pub const MAX_VIRTIO_CAPS: usize = 16;
pub const PCI_CAP_ID_VNDR: u8 = 0x09; // Vendor-Specific
pub const PCI_CONFIG_BASE_ADDR_0: u8 = 0x10; // Base Address Register 0 (BAR0)

#[derive(Debug)]
pub struct PciCapabilityTest {
    pub(crate) id: u8,
    pub(crate) offset: u8,
}

#[derive(Debug)]
pub struct PciCapability {
    /// Generic PCI field: PCI_CAP_ID_VNDR
    pub cap_vndr: u8,
    /// Generic PCI field: next ptr.
    pub cap_next: u8,
    /// Generic PCI field: capability length
    pub cap_len: u8,
    /// Identifies the structure.
    pub cfg_type: u8,
    /// Where to find it.
    pub bar: u8,
    /// Multiple capabilities of the same type.
    pub id: u8,
    /// Pad to full dword. Not used
    pub _padding: u8,
    /// Offset within the bar.
    /// Little-endian.
    pub offset: u32,
    /// Length of the structure, in bytes.
    /// Little-endian.
    pub length: u32,
}

impl PciCapability {
    /// Reads all capabilities from the PCI configuration space for the given device.
    pub fn read_all(config_space: &ConfigurationSpace, address: PciAddress) -> Vec<Self> {
        let mut capabilities = Vec::new();

        // Define offsets/constants for PCI configuration space.
        const PCI_STATUS_OFFSET: u16 = 0x06;
        const PCI_STATUS_CAP_LIST: u16 = 1 << 4;
        const PCI_CAP_POINTER_OFFSET: u16 = 0x34;

        // Check if the device supports capabilities.
        let status = config_space.read_u16(address, PCI_STATUS_OFFSET);
        if status & PCI_STATUS_CAP_LIST == 0 {
            return capabilities;
        }

        // Read the pointer to the first capability.
        let mut cap_ptr = config_space.read_u8(address, PCI_CAP_POINTER_OFFSET);
        while cap_ptr != 0 {
            let base = cap_ptr as u16;
            let cap_vndr = config_space.read_u8(address, base + 0);
            let cap_next = config_space.read_u8(address, base + 1);
            let cap_len  = config_space.read_u8(address, base + 2);
            let cfg_type = config_space.read_u8(address, base + 3);
            let bar      = config_space.read_u8(address, base + 4);
            let id       = config_space.read_u8(address, base + 5);
            let _padding = config_space.read_u8(address, base + 6);
            let offset   = config_space.read_u32(address, base + 7);
            let length   = config_space.read_u32(address, base + 11);

            capabilities.push(PciCapability {
                cap_vndr,
                cap_next,
                cap_len,
                cfg_type,
                bar,
                id,
                _padding,
                offset,
                length,
            });

            cap_ptr = cap_next;
        }

        capabilities
    }
}



pub enum CfgType {
    /// Common Configuration.
    VirtioPciCapCommonCfg = 1,
    /// Notifications.
    VirtioPciCapNotifyCfg = 2,
    /// ISR Status.
    VirtioPciCapIsrCfg = 3,
    /// Device specific configuration.
    VirtioPciCapDeviceCfg = 4,
    /// PCI configuration access.
    VirtioPciCapPciCfg = 5,
    /// Shared memory region.
    VirtioPciCapSharedMemoryCfg = 8,
    /// Vendor-specific data.
    VirtioPciCapVendorCfg = 9,
}


/// All of these values are in Little-endian.
pub struct CommonCfg {
    /// The driver uses this to select which feature bits device_feature shows.
    /// Value 0x0 selects Feature Bits 0 to 31, 0x1 selects Feature Bits 32 to 63, etc.
    /// read-write
    pub device_feature_select: u32,
    /// The device uses this to report which feature bits it is offering to the driver:
    /// the driver writes to device_feature_select to select which feature bits are presented.
    /// read-only for driver
    pub device_feature: u32,
    /// The driver uses this to select which feature bits driver_feature shows.
    /// Value 0x0 selects Feature Bits 0 to 31, 0x1 selects Feature Bits 32 to 63, etc.
    /// read-write
    pub driver_feature_select: u32,
    /// The driver writes this to accept feature bits offered by the device.
    /// Driver Feature Bits selected by driver_feature_select.
    /// read-write
    pub driver_feature: u32,
    /// The driver sets the Configuration Vector for MSI-X.
    /// read-write
    pub config_msix_vector: u32,
    /// The device specifies the maximum number of virtqueues supported here.
    /// read-only for driver
    pub num_queues: u32,
    /// The driver writes the device status here (see 2.1).
    /// Writing 0 into this field resets the device.
    /// read-write
    pub device_status: DeviceStatusFlags,
    /// Configuration atomicity value. The device changes this every time the
    /// configuration noticeably changes.
    /// read-only for driver
    pub config_generation: u8,
    /// Queue Select. The driver selects which virtqueue the following fields refer to.
    /// read-write
    pub queue_select: u16,
    /// Queue Size. On reset, specifies the maximum queue size supported by the device.
    /// This can be modified by the driver to reduce memory requirements.
    /// A 0 means the queue is unavailable.
    /// read-write
    pub queue_size: u16,
    /// The driver uses this to specify the queue vector for MSI-X.
    /// read-write
    pub queue_msix_vector: u16,
    /// The driver uses this to selectively prevent the device from executing
    /// requests from this virtqueue. 1 - enabled; 0 - disabled.
    /// read-write
    pub queue_enable: u16,
    /// The driver reads this to calculate the offset from start of Notification
    /// structure at which this virtqueue is located. Note: this is not an offset
    /// in bytes. See 4.1.4.4 below.
    /// read-only for driver
    pub queue_notify_off: u16,
    /// The driver writes the physical address of Descriptor Area here.
    /// See section 2.6.
    /// read-write
    pub queue_desc: u16,
    /// The driver writes the physical address of Driver Area here.
    /// See section 2.6.
    /// read-write
    pub queue_driver: u16,
    /// The driver writes the physical address of Device Area here.
    /// See section 2.6.
    /// read-write
    pub queue_device: u16,
    /// This field exists only if VIRTIO_F_NOTIF_CONFIG_DATA has been negotiated.
    /// The driver will use this value to put it in the ’virtqueue number’ field
    /// in the available buffer notification structure. See section 4.1.5.2. Note:
    /// This field provides the device with flexibility to determine how virtqueues
    /// will be referred to in available buffer notifications. In a trivial case the
    /// device can set queue_notify_data=vqn. Some devices may benefit from providing
    /// another value, for example an internal virtqueue identifier, or an internal
    /// offset related to the virtqueue number.
    /// read-only for driver
    pub queue_notify_data: u16,
    /// The driver uses this to selectively reset the queue. This field exists
    /// only if VIRTIO_F_RING_RESET has been negotiated. (see 2.6.1).
    /// read-write
    pub queue_reset: u16,
}

pub struct NotifyCfg {
    pub cap: PciCapability,
    /// The driver writes the queue number it is interested in to this field.
    /// read-write
    pub notify_off_multiplier: u32,
}

impl NotifyCfg {
    pub fn notify_off_multiplier(&self) -> u32 {
        self.notify_off_multiplier
    }
}