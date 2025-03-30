use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::BitOr;
use core::sync::atomic::{AtomicU16, AtomicU8};
use log::info;
use pci_types::{Bar, CommandRegister, ConfigRegionAccess, EndpointHeader, PciAddress};
use spin::{Mutex, RwLock};
use spin::rwlock::RwLockWriteGuard;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::structures::paging::page::PageRange;
use crate::device::virtio::transport::capabilities::{CommonCfgRegisters, PciCapability, MAX_VIRTIO_CAPS, PCI_CAP_ID_VNDR, VIRTIO_PCI_CAP_COMMON_CFG, VIRTIO_PCI_CAP_NOTIFY_CFG, VIRTIO_PCI_CAP_ISR_CFG, VIRTIO_PCI_CAP_DEVICE_CFG, VIRTIO_PCI_CAP_PCI_CFG, VIRTIO_PCI_CAP_SHARED_MEMORY_CFG, VIRTIO_PCI_CAP_VENDOR_CFG, CommonCfg, NotifyCfg, IsrCfg};
use crate::device::virtio::transport::dma::DmaBuffer;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::memory::{pages, MemorySpace};
use crate::{allocator, pci_bus, process_manager};
use crate::device::pci::ConfigurationSpace;
use crate::device::virtio::lib::PAGE_SIZE;
use crate::memory::vmm::VmaType;

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

pub struct VirtioGpu<'a> {
    pci_device: &'a RwLock<EndpointHeader>,
    cap_ptr: PciAddress,
    irq: i32,
    
    virtio_caps: Vec<PciCapability>, 
    virtio_caps_count: u32,
    common_cfg: CommonCfg,
    isr_cfg: IsrCfg,
    notify_cfg: NotifyCfg,
    config_ptr: u32,

    //rect: Mutex<VirtioGpuRect>,
    rect: u32, // testing
    //frame_buffer: Mutex<DmaBuffer>,
    frame_buffer: u32, // testing
    // cursot_buffer not implemented
    //control_queue: VirtioQueue,
    queue_buffer_send: Box<[u8]>,
    queue_buffer_recv: Box<[u8]>,
}

pub struct VirtioGpuInterruptHandler<'a> {
    device: Arc<VirtioGpu<'a>>,
}

#[repr(C)]
struct GpuConfig {
    /// signals pending events to the driver. The driver MUST NOT write to this field.
    events_read: u32,
    /// clears pending events in the device. Writing a ’1’ into a bit will clear the corresponding bit in events_read, mimicking write-to-clear behavior.
    events_clear: u32,
    /// specifies the maximum number of scanouts supported by the device. Minimum value is 1, maximum value is 16.
    num_scanouts: u32,
    /// specifies the maximum number of capability sets supported by the device. The minimum value is zero.
    num_capsets: u32,
}

#[repr(u32)]
enum VirtioGpuCtrlType {
    Undefined = 0,

    // 2D commands
    GetDisplayInfo = 0x0100,
    ResourceCreate2d,
    ResourceUnref,
    SetScanout,
    ResourceFlush,
    TransferToHost2d,
    ResourceAttachBacking,
    ResourceDetachBacking,
    GetCapsetInfo,
    GetCapset,
    GetEdid,
    ResourceAssignUuid,
    ResourceCreateBlob,
    SetScanoutBlob,

    // 3D commands
    CtxCreate = 0x0200,
    CtxDestroy,
    CtxAttachResource,
    CtxDetachResource,
    ResourceCreate3d,
    TransferToHost3d,
    TransferFromHost3d,
    Submit3d,
    ResourceMapBlob,
    ResourceUnmapBlob,

    // cursor commands
    UpdateCursor = 0x0300,
    MoveCursor,

    // success responses
    RespOkNodata = 0x1100,
    RespOkDisplayInfo,
    RespOkCapsetInfo,
    RespOkCapset,
    RespOkEdid,
    RespOkResourceUuid,
    RespOkMapInfo,

    // error responses
    RespErrUnspec = 0x1200,
    RespErrOutOfMemory,
    RespErrInvalidScanoutId,
    RespErrInvalidResourceId,
    RespErrInvalidContextId,
    RespErrInvalidParameter,
}

const VIRTIO_GPU_FLAG_FENCE: u32 = 1 << 0;
const VIRTIO_GPU_FLAG_INFO_RING_IDX: u32 = 1 << 1;

#[repr(C)]
struct VirtioGpuCtrlHdr {
    type_: VirtioGpuCtrlType,
    flags: u32,
    fence_id: u64,
    ctx_id: u32,
    ring_idx: u8,
    _padding: u32,
}

impl VirtioGpuCtrlHdr {
    fn with_type(type_: VirtioGpuCtrlType) -> Self {
        Self {
            type_,
            flags: 0,
            fence_id: 0,
            ctx_id: 0,
            ring_idx: 0,
            _padding: 0,
        }
    }

    /*fn check_type(&self, type_: VirtioGpuCtrlType) -> bool {
        self.type_ == type_
    }*/
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VirtioGpuRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
struct VirtioGpuRespDisplayInfo {
    hdr: VirtioGpuCtrlHdr,
    rect: VirtioGpuRect,
    enabled: u32,
    flags: u32,
}

#[repr(u32)]
enum VirtioGpuFormats {
    B8G8R8A8UNORM = 1,
}

#[repr(C)]
struct ResourceCreate2d {
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    format: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
struct VirtioGpuSetScanout {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    scanout_id: u32,
    resource_id: u32,
}

#[repr(C)]
struct VirtioGpuResourceFlush {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    resource_id: u32,
    _padding: u32,
}

#[repr(C)]
struct VirtioGpuTransferToHost2d {
    hdr: VirtioGpuCtrlHdr,
    r: VirtioGpuRect,
    offset: u64,
    resource_id: u32,
    _padding: u32,
}

#[repr(C)]
struct VirtioGpuResourceAttachBacking {
    hdr: VirtioGpuCtrlHdr,
    resource_id: u32,
    nr_entries: u32,
    addr: u64,
    len: u32,
    _padding: u32,
}

// Cursor Structs not implemented
const QUEUE_TRANSMIT: u16 = 0;
const SCANOUT_ID: u32 = 0;
const RESOURCE_ID_FB: u32 = 0xbabe;


impl<'a> VirtioGpu<'a> {
    pub fn new(pci_device: &'a RwLock<EndpointHeader>) -> Result<Self, String> {

        // This is the address of the device where the PCI capabilities are located
        let cap_ptr = pci_device.read().header().address();

        let (common_cfg, notify_cfg, isr_cfg,virtio_caps) = Self::extract_capabilities(pci_device, cap_ptr)?;
        let virtio_caps_count = virtio_caps.len() as u32;

        info!("Isr_cfg status: {:?}", isr_cfg.read_status());

        Ok(VirtioGpu {
            pci_device,
            cap_ptr,
            irq: 0,
            virtio_caps,
            virtio_caps_count,
            common_cfg,
            isr_cfg,
            notify_cfg,
            config_ptr: 0,
            rect: 0,
            frame_buffer: 0,
            queue_buffer_send: Box::new([]),
            queue_buffer_recv: Box::new([]),
        })
    }

    fn extract_capabilities(pci_device: &'a RwLock<EndpointHeader>, cap_ptr: PciAddress) -> Result<(CommonCfg, NotifyCfg, IsrCfg, Vec<PciCapability>), String> {
        info!("Configuring PCI registers");
        let pci_config_space = pci_bus().config_space();
        let mut pci_device = pci_device.write();


        // Read the PCI configuration space
        let virtio_caps = PciCapability::read_all(pci_config_space, cap_ptr);
        let mut common_cfg = None;
        let mut notify_cfg = None;
        let mut isr_cfg = None;

        for cap in virtio_caps.iter() {
            match cap.cfg_type {
                VIRTIO_PCI_CAP_COMMON_CFG => {
                    common_cfg = PciCapability::extract_common_cfg(&pci_config_space, &mut pci_device, cap);
                    if common_cfg.is_none() {
                        return Err("Failed to extract common configuration".to_string());
                    }

                    info!("Found common configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                },
                VIRTIO_PCI_CAP_NOTIFY_CFG => {
                    notify_cfg = PciCapability::extract_notify_cfg(&pci_config_space, &mut pci_device, cap);
                    if notify_cfg.is_none() {
                        return Err("Failed to extract notify configuration".to_string());
                    }

                    info!("Found notify configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                },
                VIRTIO_PCI_CAP_ISR_CFG => {
                    isr_cfg = PciCapability::extract_isr_cfg(&pci_config_space, &mut pci_device, cap);
                    if isr_cfg.is_none() {
                        return Err("Failed to extract ISR configuration".to_string());
                    }

                    info!("Found ISR configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                },
                VIRTIO_PCI_CAP_DEVICE_CFG => {
                    info!("Found device configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                    // Handle device configuration
                },
                VIRTIO_PCI_CAP_SHARED_MEMORY_CFG => {
                    info!("Found shared memory configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                    // Handle shared memory configuration
                },
                VIRTIO_PCI_CAP_VENDOR_CFG => {
                    info!("Found vendor-specific configuration capability at bar: {}, offset: {}", cap.bar, cap.offset);
                    // Handle vendor-specific configuration
                },
                _ => {
                    info!("Found unknown configuration capability: {:?}", cap.cfg_type);
                },
            }
        }

        let common_cfg = common_cfg.ok_or("Common configuration not found")?;
        let notify_cfg = notify_cfg.ok_or("Notify configuration not found")?;
        let isr_cfg = isr_cfg.ok_or("ISR configuration not found")?;

        Ok((common_cfg, notify_cfg, isr_cfg, virtio_caps))
    }
}




/*impl VirtioGpuDevice {
    pub fn new(device: Arc<Mutex<EndpointHeader>>) -> Self {
        //let io_base = device.lock().bar(0, ()) as u16;
        //let irq = device.lock().interrupt_line();
        Self { device, io_base, irq }
    }

    pub fn init(&self) {
        info!("Initializing Virtio GPU driver...");
        self.reset_device();
        self.setup_features();
        self.setup_queues();
        self.enable_interrupts();
    }

    fn reset_device(&self) {
        let mut status_port = Port::<u32>::new(self.io_base + 0x14);
        unsafe { status_port.write(0); }
    }

    fn setup_features(&self) {
        let mut features_port = Port::<u32>::new(self.io_base + 0x10);
        let features = unsafe { features_port.read() };
        info!("Device features: {:#X}", features);
    }

    fn setup_queues(&self) {
        info!("Setting up Virtqueues...");
    }

    fn enable_interrupts(&self) {
        info!("Enabling interrupts for Virtio GPU...");
    }
}*/

/*
// GPU Commands
#[repr(C)]
pub struct VirtioGpuCtrlHdr {
    // Command header fields
}

#[repr(C)]
pub struct VirtioGpuResourceCreate2d {
    // Fields for resource creation
}

#[repr(C)]
pub struct VirtioGpuResourceFlush {
    // Fields for resource flush
}*/