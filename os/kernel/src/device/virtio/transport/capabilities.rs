use alloc::format;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::ops::Deref;
use core::ptr::NonNull;
use pci_types::{EndpointHeader, PciAddress};
use spin::Mutex;
use spin::rwlock::RwLockWriteGuard;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::register_bitfields;
use tock_registers::registers::{ReadOnly, ReadWrite};
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::device::pci::ConfigurationSpace;
use crate::device::virtio::lib::PAGE_SIZE;
use crate::memory::MemorySpace;
use crate::memory::vmm::VmaType;
use crate::process_manager;

pub const MAX_VIRTIO_CAPS: usize = 16;
pub const PCI_CAP_ID_VNDR: u8 = 0x09; // Vendor-Specific
pub const PCI_CONFIG_BASE_ADDR_0: u8 = 0x10; // BAR0

// PCI Capability IDs
pub const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
pub const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
pub const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;
pub const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;
pub const VIRTIO_PCI_CAP_PCI_CFG: u8 = 5;
pub const VIRTIO_PCI_CAP_SHARED_MEMORY_CFG: u8 = 8;
pub const VIRTIO_PCI_CAP_VENDOR_CFG: u8 = 9;

#[derive(Debug, Clone)]
pub struct PciCapability {
    /// Generic PCI field: PCI_CAP_ID_VNDR
    pub cap_vndr: u8,
    /// Generic PCI field: next pointer.
    pub cap_next: u8,
    /// Generic PCI field: capability length.
    pub cap_len: u8,
    /// Identifies the structure.
    pub cfg_type: u8,
    /// Where to find it.
    pub bar: u8,
    /// Multiple capabilities of the same type.
    pub id: u8,
    /// Padding to full dword. Not used.
    pub _padding: u8,
    /// Offset within the BAR.
    pub offset: u32,
    /// Length of the structure, in bytes.
    pub length: u32,
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

// 16-bit register bitfields (le16 fields)
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
    pub device_feature_select: ReadWrite<u32, DEVICE_FEATURE_SELECT::Register>,
    pub device_feature: ReadOnly<u32, DEVICE_FEATURE::Register>,
    pub driver_feature_select: ReadWrite<u32, DRIVER_FEATURE_SELECT::Register>,
    pub driver_feature: ReadWrite<u32, DRIVER_FEATURE::Register>,

    // 16-bit fields
    pub config_msix_vector: ReadWrite<u16, CONFIG_MSIX_VECTOR::Register>,
    pub num_queues: ReadOnly<u16, NUM_QUEUES::Register>,

    // 8-bit fields
    pub device_status: ReadWrite<u8, DEVICE_STATUS::Register>,
    pub config_generation: ReadOnly<u8, CONFIG_GENERATION::Register>,

    // Queue-specific 16-bit fields
    pub queue_select: ReadWrite<u16, QUEUE_SELECT::Register>,
    pub queue_size: ReadWrite<u16, QUEUE_SIZE::Register>,
    pub queue_msix_vector: ReadWrite<u16, QUEUE_MSIX_VECTOR::Register>,
    pub queue_enable: ReadWrite<u16, QUEUE_ENABLE::Register>,
    pub queue_notify_off: ReadOnly<u16, QUEUE_NOTIFY_OFF::Register>,
    pub queue_notify_data: ReadOnly<u16, QUEUE_NOTIFY_DATA::Register>,
    pub queue_reset: ReadWrite<u16, QUEUE_RESET::Register>,

    // Queue-specific 64-bit address fields
    pub queue_desc: ReadWrite<u64, QUEUE_DESC::Register>,
    pub queue_driver: ReadWrite<u64, QUEUE_DRIVER::Register>,
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

    // 32-bit register accessors
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

    // 16-bit register accessors
    pub fn read_config_msix_vector(&self) -> u16 {
        self.regs().config_msix_vector.read(CONFIG_MSIX_VECTOR::VALUE)
    }
    pub fn write_config_msix_vector(&self, value: u16) {
        self.regs().config_msix_vector.write(CONFIG_MSIX_VECTOR::VALUE.val(value));
    }
    pub fn read_num_queues(&self) -> u16 {
        self.regs().num_queues.read(NUM_QUEUES::VALUE)
    }

    // 8-bit register accessors
    pub fn read_device_status(&self) -> u8 {
        self.regs().device_status.read(DEVICE_STATUS::VALUE)
    }
    pub fn write_device_status(&self, value: u8) {
        self.regs().device_status.write(DEVICE_STATUS::VALUE.val(value));
    }
    pub fn read_config_generation(&self) -> u8 {
        self.regs().config_generation.read(CONFIG_GENERATION::VALUE)
    }

    // Queue-specific 16-bit accessors
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

    // Queue-specific 64-bit accessors
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

unsafe impl Send for CommonCfg {}
unsafe impl Sync for CommonCfg {}

#[derive(Debug, Clone)]
pub struct VirtioPciNotifyCap {
    pub cap: PciCapability,
    /// The notify multiplier used for calculating per-queue notify addresses.
    pub notify_off_multiplier: u32,
}

// --- Global BAR mapping cache ---
// This cache maps a (device address, BAR index) to a virtual base address.
static BAR_MAPPINGS: Mutex<BTreeMap<(PciAddress, u8), u64>> = Mutex::new(BTreeMap::new());

/// Maps a BAR only once for the given PCI device and returns the virtual base address.
/// Assumes identity mapping (i.e. virtual == physical); adjust if needed.
fn map_bar_once(
    pci_config_space: &ConfigurationSpace,
    pci_device: &mut RwLockWriteGuard<EndpointHeader>,
    device_addr: PciAddress,
    bar_index: u8,
) -> u64 {
    let key = (device_addr, bar_index);
    {
        let mappings = BAR_MAPPINGS.lock();
        if let Some(&virt_base) = mappings.get(&key) {
            return virt_base;
        }
    }
    let bar = pci_device
        .bar(bar_index, pci_config_space)
        .expect("Failed to read BAR");
    let base_address = bar.unwrap_mem();
    let phys_addr = base_address.0 as u64;
    let size = base_address.1;
    let start_page = Page::from_start_address(VirtAddr::new(phys_addr))
        .expect("BAR address is not page aligned");
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
            &format!("bar_{}", bar_index),
        );
    // For simplicity we assume identity mapping.
    let virt_base = phys_addr;
    BAR_MAPPINGS.lock().insert(key, virt_base);
    virt_base
}

impl PciCapability {
    /// Reads all PCI capabilities from the configuration space for the given device.
    pub fn read_all(config_space: &ConfigurationSpace, address: PciAddress) -> Vec<Self> {
        let mut capabilities = Vec::new();
        const PCI_STATUS_OFFSET: u16 = 0x06;
        const PCI_STATUS_CAP_LIST: u16 = 1 << 4;
        const PCI_CAP_POINTER_OFFSET: u16 = 0x34;
        let status = config_space.read_u16(address, PCI_STATUS_OFFSET);
        if status & PCI_STATUS_CAP_LIST == 0 {
            return capabilities;
        }
        let mut cap_ptr = config_space.read_u8(address, PCI_CAP_POINTER_OFFSET);
        let mut virtio_caps_count = 0;
        while cap_ptr != 0 && virtio_caps_count < MAX_VIRTIO_CAPS {
            let base = cap_ptr as u16;
            let cap_vndr = config_space.read_u8(address, base + 0);
            let cap_next = config_space.read_u8(address, base + 1);
            let cap_len  = config_space.read_u8(address, base + 2);
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

    /// Extracts the common configuration structure from the device.
    pub fn extract_common_cfg(
        pci_config_space: &ConfigurationSpace,
        pci_device: &mut RwLockWriteGuard<EndpointHeader>,
        cap: &PciCapability
    ) -> Option<CommonCfg> {
        let device_addr = pci_device.header().address();
        let virt_base = map_bar_once(pci_config_space, pci_device, device_addr, cap.bar);
        let common_cfg_ptr = (virt_base + cap.offset as u64) as *mut CommonCfgRegisters;
        Some(unsafe { CommonCfg::new(common_cfg_ptr) })
    }

    /// Extracts the notify capability structure from the device.
    pub fn extract_notify_cfg(
        pci_config_space: &ConfigurationSpace,
        pci_device: &mut RwLockWriteGuard<EndpointHeader>,
        cap: &PciCapability,
    ) -> Option<VirtioPciNotifyCap> {
        let device_addr = pci_device.header().address();
        let virt_base = map_bar_once(pci_config_space, pci_device, device_addr, cap.bar);
        // The notify_off_multiplier is located at an offset of 8 bytes from the start of the notify capability.
        let notify_addr = virt_base + cap.offset as u64 + 8;
        let notify_off_multiplier = unsafe { core::ptr::read_volatile(notify_addr as *const u32) };
        Some(VirtioPciNotifyCap {
            cap: cap.clone(),
            notify_off_multiplier,
        })
    }
}
