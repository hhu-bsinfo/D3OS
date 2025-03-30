use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr::NonNull;
use log::info;
use pci_types::{EndpointHeader, PciAddress};
use spin::Mutex;
use spin::rwlock::RwLockWriteGuard;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_bitfields;
use tock_registers::registers::{ReadOnly, ReadWrite};
use x86_64::instructions::port::{Port, PortReadOnly};
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::device::pci::ConfigurationSpace;
use crate::device::virtio::lib::PAGE_SIZE;
use crate::device::virtio::transport::flags::DeviceStatusFlags;
use crate::memory::MemorySpace;
use crate::memory::vmm::VmaType;
use crate::process_manager;

pub const MAX_VIRTIO_CAPS: usize = 16;
pub const PCI_CAP_ID_VNDR: u8 = 0x09; // Vendor-Specific
pub const PCI_CONFIG_BASE_ADDR_0: u8 = 0x10; // Base Address Register 0 (BAR0)

// PCI Capability IDs
pub const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
pub const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
pub const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
pub const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;
pub const VIRTIO_PCI_CAP_PCI_CFG: u8 = 5;
pub const VIRTIO_PCI_CAP_SHARED_MEMORY_CFG: u8 = 8;
pub const VIRTIO_PCI_CAP_VENDOR_CFG: u8 = 9;

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
    pub offset: u32,
    /// Length of the structure, in bytes.
    pub length: u32,
}

impl PciCapability {
    /// Reads all capabilities from the PCI configuration space for the given device.
    pub fn read_capabilities(config_space: &ConfigurationSpace, address: PciAddress) -> Vec<Self> {
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
        let mut virtio_caps_count = 0;
        while cap_ptr != 0 && virtio_caps_count < MAX_VIRTIO_CAPS {
            let base = cap_ptr as u16;
            let cap_vndr = config_space.read_u8(address, base + 0);
            let cap_next = config_space.read_u8(address, base + 1);
            let cap_len  = config_space.read_u8(address, base + 2);

            // Check if the capability is a vendor-specific capability.
            if cap_vndr == PCI_CAP_ID_VNDR {
                let cfg_type = config_space.read_u8(address, base + 3);
                let bar = config_space.read_u8(address, base + 4);
                let id = config_space.read_u8(address, base + 5);
                let offset = config_space.read_u32(address, base + 8);
                let length = config_space.read_u32(address, base + 12);

                let pci_capability = PciCapability {
                    cap_vndr,
                    cap_next,
                    cap_len,
                    cfg_type,
                    bar,
                    id,
                    _padding: 0,
                    offset,
                    length,
                };


                capabilities.push(pci_capability);
            }

            cap_ptr = cap_next;
            virtio_caps_count += 1;
        }

        capabilities
    }

    pub fn extract_common_cfg(pci_config_space: &&ConfigurationSpace, pci_device: &mut RwLockWriteGuard<EndpointHeader>, cap: &PciCapability) -> &'static CommonCfg {
        let bar = pci_device.bar(cap.bar, &pci_config_space).expect("Failed to read BAR");
        let base_address = bar.unwrap_mem();

        let address = base_address.0 as u64;
        let size = base_address.1;

        let start_page = Page::from_start_address(VirtAddr::new(address)).unwrap();
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        process_manager()
            .read()
            .kernel_process()
            .expect("Failed to get kernel process")
            .virtual_address_space
            .map(
                PageRange {
                    start: start_page,
                    end: start_page + num_pages as u64,
                },
                MemorySpace::Kernel,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                VmaType::DeviceMemory,
                "common_cfg",
            );

        // Initialize the CommonCfg struct
        let common_cfg_ptr = (address + cap.offset as u64) as *mut CommonCfgRegisters;
        let common_cfg = unsafe { CommonCfg::new(common_cfg_ptr) };
        &common_cfg
    }
}

// 32-bit register bitfields
register_bitfields![u32,
    DEVICE_FEATURE_SELECT [
        VALUE OFFSET(0) NUMBITS(32) []
    ],
    DEVICE_FEATURE [
        VALUE OFFSET(0) NUMBITS(32) []
    ],
    DRIVER_FEATURE_SELECT [
        VALUE OFFSET(0) NUMBITS(32) []
    ],
    DRIVER_FEATURE [
        VALUE OFFSET(0) NUMBITS(32) []
    ]
];

// 16-bit register bitfields for fields defined as le16
register_bitfields![u16,
    CONFIG_MSIX_VECTOR [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    NUM_QUEUES [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_SELECT [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_SIZE [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_MSIX_VECTOR [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_ENABLE [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_NOTIFY_OFF [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_NOTIFY_DATA [
        VALUE OFFSET(0) NUMBITS(16) []
    ],
    QUEUE_RESET [
        VALUE OFFSET(0) NUMBITS(16) []
    ]
];

// 8-bit register bitfields
register_bitfields![u8,
    DEVICE_STATUS [
        VALUE OFFSET(0) NUMBITS(8) []
    ],
    CONFIG_GENERATION [
        VALUE OFFSET(0) NUMBITS(8) []
    ]
];

// 64-bit register bitfields for queue addresses
register_bitfields![u64,
    QUEUE_DESC [
        VALUE OFFSET(0) NUMBITS(64) []
    ],
    QUEUE_DRIVER [
        VALUE OFFSET(0) NUMBITS(64) []
    ],
    QUEUE_DEVICE [
        VALUE OFFSET(0) NUMBITS(64) []
    ]
];

#[repr(C)]
pub struct CommonCfgRegisters {
    // 32-bit fields
    /// Driver selects which feature bits device_feature shows.
    pub device_feature_select: ReadWrite<u32, DEVICE_FEATURE_SELECT::Register>,
    /// Device reports its offered features.
    pub device_feature: ReadOnly<u32, DEVICE_FEATURE::Register>,
    /// Driver selects which feature bits driver_feature shows.
    pub driver_feature_select: ReadWrite<u32, DRIVER_FEATURE_SELECT::Register>,
    /// Driver writes accepted features.
    pub driver_feature: ReadWrite<u32, DRIVER_FEATURE::Register>,

    // 16-bit fields
    /// Configuration Vector for MSI-X.
    pub config_msix_vector: ReadWrite<u16, CONFIG_MSIX_VECTOR::Register>,
    /// Maximum number of virtqueues supported.
    pub num_queues: ReadOnly<u16, NUM_QUEUES::Register>,

    // 8-bit fields
    /// Device status (writing 0 resets the device).
    pub device_status: ReadWrite<u8, DEVICE_STATUS::Register>,
    /// Configuration atomicity value.
    pub config_generation: ReadOnly<u8, CONFIG_GENERATION::Register>,

    // Queue-specific fields (16-bit)
    /// Selects which virtqueue the following fields refer to.
    pub queue_select: ReadWrite<u16, QUEUE_SELECT::Register>,
    /// Maximum queue size supported by the device.
    pub queue_size: ReadWrite<u16, QUEUE_SIZE::Register>,
    /// Queue vector for MSI-X.
    pub queue_msix_vector: ReadWrite<u16, QUEUE_MSIX_VECTOR::Register>,
    /// Enables/disables the virtqueue.
    pub queue_enable: ReadWrite<u16, QUEUE_ENABLE::Register>,
    /// Offset in the Notification structure.
    pub queue_notify_off: ReadOnly<u16, QUEUE_NOTIFY_OFF::Register>,
    /// Virtqueue notification configuration data.
    pub queue_notify_data: ReadOnly<u16, QUEUE_NOTIFY_DATA::Register>,
    /// Used to reset the queue (if negotiated).
    pub queue_reset: ReadWrite<u16, QUEUE_RESET::Register>,

    // Queue-specific fields (64-bit addresses)
    /// Physical address of Descriptor Area.
    pub queue_desc: ReadWrite<u64, QUEUE_DESC::Register>,
    /// Physical address of Driver Area.
    pub queue_driver: ReadWrite<u64, QUEUE_DRIVER::Register>,
    /// Physical address of Device Area.
    pub queue_device: ReadWrite<u64, QUEUE_DEVICE::Register>,
}


pub struct CommonCfg {
    regs: NonNull<CommonCfgRegisters>,
}

impl CommonCfg {
    /// # Safety
    /// `base_addr` must point to a valid memory-mapped device register block.
    pub unsafe fn new(base_addr: *mut CommonCfgRegisters) -> Self {
        Self {
            regs: NonNull::new(base_addr).expect("null pointer to device registers"),
        }
    }

    #[inline]
    fn regs(&self) -> &CommonCfgRegisters {
        unsafe { self.regs.as_ref() }
    }

    // 32-bit registers
    pub fn read_device_feature_select(&self) -> u32 {
        self.regs().device_feature_select.read(DEVICE_FEATURE_SELECT::VALUE)
    }

    pub fn write_device_feature_select(&self, value: u32) {
        self.regs().device_feature_select.write(DEVICE_FEATURE_SELECT::VALUE.val(value));
    }

    pub fn read_device_feature(&self) -> u32 {
        self.regs().device_feature.read(DEVICE_FEATURE::VALUE)
    }

    pub fn read_driver_feature_select(&self) -> u32 {
        self.regs().driver_feature_select.read(DRIVER_FEATURE_SELECT::VALUE)
    }

    pub fn write_driver_feature_select(&self, value: u32) {
        self.regs().driver_feature_select.write(DRIVER_FEATURE_SELECT::VALUE.val(value));
    }

    pub fn read_driver_feature(&self) -> u32 {
        self.regs().driver_feature.read(DRIVER_FEATURE::VALUE)
    }

    pub fn write_driver_feature(&self, value: u32) {
        self.regs().driver_feature.write(DRIVER_FEATURE::VALUE.val(value));
    }

    // 16-bit registers
    pub fn read_config_msix_vector(&self) -> u16 {
        self.regs().config_msix_vector.read(CONFIG_MSIX_VECTOR::VALUE)
    }

    pub fn write_config_msix_vector(&self, value: u16) {
        self.regs().config_msix_vector.write(CONFIG_MSIX_VECTOR::VALUE.val(value));
    }

    pub fn read_num_queues(&self) -> u16 {
        self.regs().num_queues.read(NUM_QUEUES::VALUE)
    }

    // 8-bit registers
    pub fn read_device_status(&self) -> u8 {
        self.regs().device_status.read(DEVICE_STATUS::VALUE)
    }

    pub fn write_device_status(&self, value: u8) {
        self.regs().device_status.write(DEVICE_STATUS::VALUE.val(value));
    }

    pub fn read_config_generation(&self) -> u8 {
        self.regs().config_generation.read(CONFIG_GENERATION::VALUE)
    }

    // Queue-specific 16-bit registers
    pub fn read_queue_select(&self) -> u16 {
        self.regs().queue_select.read(QUEUE_SELECT::VALUE)
    }

    pub fn write_queue_select(&self, value: u16) {
        self.regs().queue_select.write(QUEUE_SELECT::VALUE.val(value));
    }

    pub fn read_queue_size(&self) -> u16 {
        self.regs().queue_size.read(QUEUE_SIZE::VALUE)
    }

    pub fn write_queue_size(&self, value: u16) {
        self.regs().queue_size.write(QUEUE_SIZE::VALUE.val(value));
    }

    pub fn read_queue_msix_vector(&self) -> u16 {
        self.regs().queue_msix_vector.read(QUEUE_MSIX_VECTOR::VALUE)
    }

    pub fn write_queue_msix_vector(&self, value: u16) {
        self.regs().queue_msix_vector.write(QUEUE_MSIX_VECTOR::VALUE.val(value));
    }

    pub fn read_queue_enable(&self) -> u16 {
        self.regs().queue_enable.read(QUEUE_ENABLE::VALUE)
    }

    pub fn write_queue_enable(&self, value: u16) {
        self.regs().queue_enable.write(QUEUE_ENABLE::VALUE.val(value));
    }

    pub fn read_queue_notify_off(&self) -> u16 {
        self.regs().queue_notify_off.read(QUEUE_NOTIFY_OFF::VALUE)
    }

    pub fn read_queue_notify_data(&self) -> u16 {
        self.regs().queue_notify_data.read(QUEUE_NOTIFY_DATA::VALUE)
    }

    pub fn read_queue_reset(&self) -> u16 {
        self.regs().queue_reset.read(QUEUE_RESET::VALUE)
    }

    pub fn write_queue_reset(&self, value: u16) {
        self.regs().queue_reset.write(QUEUE_RESET::VALUE.val(value));
    }

    // Queue-specific 64-bit registers
    pub fn read_queue_desc(&self) -> u64 {
        self.regs().queue_desc.read(QUEUE_DESC::VALUE)
    }

    pub fn write_queue_desc(&self, value: u64) {
        self.regs().queue_desc.write(QUEUE_DESC::VALUE.val(value));
    }

    pub fn read_queue_driver(&self) -> u64 {
        self.regs().queue_driver.read(QUEUE_DRIVER::VALUE)
    }

    pub fn write_queue_driver(&self, value: u64) {
        self.regs().queue_driver.write(QUEUE_DRIVER::VALUE.val(value));
    }

    pub fn read_queue_device(&self) -> u64 {
        self.regs().queue_device.read(QUEUE_DEVICE::VALUE)
    }

    pub fn write_queue_device(&self, value: u64) {
        self.regs().queue_device.write(QUEUE_DEVICE::VALUE.val(value));
    }
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