use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly};
use crate::device::virtio::transport::flags::DeviceStatusFlags;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct PciCapability {
    /// Generic PCI field: PCI_CAP_ID_VNDR
    pub cap_vndr: u8,
    /// Generic PCI field: next ptr.
    pub cap_next: u8,
    /// Generic PCI field: capability length
    pub cap_len: u8,
    /// Identifies the structure.
    pub cfg_type: CfgType,
    /// Where to find it.
    pub bar: u8,
    /// Multiple capabilities of the same type.
    pub id: u8,
    /// Offset within the bar.
    /// Little-endian.
    pub offset: u32,
    /// Length of the structure, in bytes.
    /// Little-endian.
    pub length: u32,
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
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
#[derive(Debug)]
#[repr(C)]
pub struct CommonCfg {
    /// The driver uses this to select which feature bits device_feature shows.
    /// Value 0x0 selects Feature Bits 0 to 31, 0x1 selects Feature Bits 32 to 63, etc.
    /// read-write
    pub device_feature_select: Mutex<Port<u32>>,
    /// The device uses this to report which feature bits it is offering to the driver:
    /// the driver writes to device_feature_select to select which feature bits are presented.
    /// read-only for driver
    pub device_feature: PortReadOnly<u32>,
    /// The driver uses this to select which feature bits driver_feature shows.
    /// Value 0x0 selects Feature Bits 0 to 31, 0x1 selects Feature Bits 32 to 63, etc.
    /// read-write
    pub driver_feature_select: Mutex<Port<u32>>,
    /// The driver writes this to accept feature bits offered by the device.
    /// Driver Feature Bits selected by driver_feature_select.
    /// read-write
    pub driver_feature: Mutex<Port<u32>>,
    /// The driver sets the Configuration Vector for MSI-X.
    /// read-write
    pub config_msix_vector: Mutex<Port<u32>>,
    /// The device specifies the maximum number of virtqueues supported here.
    /// read-only for driver
    pub num_queues: PortReadOnly<u32>,
    /// The driver writes the device status here (see 2.1).
    /// Writing 0 into this field resets the device.
    /// read-write
    pub device_status: Mutex<Port<DeviceStatusFlags>>,
    /// Configuration atomicity value. The device changes this every time the
    /// configuration noticeably changes.
    /// read-only for driver
    pub config_generation: PortReadOnly<u8>,
    /// Queue Select. The driver selects which virtqueue the following fields refer to.
    /// read-write
    pub queue_select: Mutex<Port<u16>>,
    /// Queue Size. On reset, specifies the maximum queue size supported by the device.
    /// This can be modified by the driver to reduce memory requirements.
    /// A 0 means the queue is unavailable.
    /// read-write
    pub queue_size: Mutex<Port<u16>>,
    /// The driver uses this to specify the queue vector for MSI-X.
    /// read-write
    pub queue_msix_vector: Mutex<Port<u16>>,
    /// The driver uses this to selectively prevent the device from executing
    /// requests from this virtqueue. 1 - enabled; 0 - disabled.
    /// read-write
    pub queue_enable: Mutex<Port<u16>>,
    /// The driver reads this to calculate the offset from start of Notification
    /// structure at which this virtqueue is located. Note: this is not an offset
    /// in bytes. See 4.1.4.4 below.
    /// read-only for driver
    pub queue_notify_off: PortReadOnly<u16>,
    /// The driver writes the physical address of Descriptor Area here.
    /// See section 2.6.
    /// read-write
    pub queue_desc: Mutex<Port<u16>>,
    /// The driver writes the physical address of Driver Area here.
    /// See section 2.6.
    /// read-write
    pub queue_driver: Mutex<Port<u16>>,
    /// The driver writes the physical address of Device Area here.
    /// See section 2.6.
    /// read-write
    pub queue_device: Mutex<Port<u16>>,
    /// This field exists only if VIRTIO_F_NOTIF_CONFIG_DATA has been negotiated.
    /// The driver will use this value to put it in the ’virtqueue number’ field
    /// in the available buffer notification structure. See section 4.1.5.2. Note:
    /// This field provides the device with flexibility to determine how virtqueues
    /// will be referred to in available buffer notifications. In a trivial case the
    /// device can set queue_notify_data=vqn. Some devices may benefit from providing
    /// another value, for example an internal virtqueue identifier, or an internal
    /// offset related to the virtqueue number.
    /// read-only for driver
    pub queue_notify_data: PortReadOnly<u16>,
    /// The driver uses this to selectively reset the queue. This field exists
    /// only if VIRTIO_F_RING_RESET has been negotiated. (see 2.6.1).
    /// read-write
    pub queue_reset: Mutex<Port<u16>>,
}

impl CommonCfg {
    pub fn new(base: u16) -> Self {
        Self {
            device_feature_select: Mutex::new(Port::new(base)),
            device_feature: PortReadOnly::new(base + 0x04),
            driver_feature_select: Mutex::new(Port::new(base + 0x08)),
            driver_feature: Mutex::new(Port::new(base + 0x0C)),
            config_msix_vector: Mutex::new(Port::new(base + 0x10)),
            num_queues: PortReadOnly::new(base + 0x14),
            device_status: Mutex::new(Port::new(base + 0x18)),
            config_generation: PortReadOnly::new(base + 0x19),
            queue_select: Mutex::new(Port::new(base + 0x1A)),
            queue_size: Mutex::new(Port::new(base + 0x1C)),
            queue_msix_vector: Mutex::new(Port::new(base + 0x1E)),
            queue_enable: Mutex::new(Port::new(base + 0x20)),
            queue_notify_off: PortReadOnly::new(base + 0x22),
            queue_desc: Mutex::new(Port::new(base + 0x24)),
            queue_driver: Mutex::new(Port::new(base + 0x26)),
            queue_device: Mutex::new(Port::new(base + 0x28)),
            queue_notify_data: PortReadOnly::new(base + 0x2A),
            queue_reset: Mutex::new(Port::new(base + 0x2C)),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
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