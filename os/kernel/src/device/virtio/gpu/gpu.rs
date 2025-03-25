use alloc::boxed::Box;
use alloc::sync::Arc;
use core::ops::BitOr;
use core::ptr::NonNull;
use core::sync::atomic::AtomicU8;
use log::info;
use pci_types::{CommandRegister, EndpointHeader};
use spin::{RwLock};
use crate::device::virtio::transport::capabilities::{CommonCfg, PciCapability};
use crate::device::virtio::transport::dma::DmaBuffer;
use crate::interrupt::interrupt_dispatcher::InterruptVector;
use crate::pci_bus;

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

pub struct VirtioGpu {
    pci_device: u32,
    cap_ptr: u8,
    irq: i32,

    virtio_caps: [PciCapability; 16],
    common_cfg: Option<NonNull<CommonCfg>>,
    common_len: usize,
    isr: Option<NonNull<AtomicU8>>,
    notify_ptr: Option<NonNull<u16>>,
    notify_off_multiplier: u32,
    config: Option<NonNull<u32>>,
    config_len: usize,

    rect: Option<VirtioGpuRect>,
    frame_buffer: Option<DmaBuffer>,
    // cursot_buffer not implemented
    //control_queue: VirtioQueue,
    queue_buffer_send: Box<[u8]>,
    queue_buffer_recv: Box<[u8]>,
}

pub struct VirtioGpuInterruptHandler {
    device: Arc<VirtioGpu>,
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


impl VirtioGpu {
    pub fn new(pci_device: &RwLock<EndpointHeader>) -> Self {
        info!("Reading Capabilities");
        let pci_config_space = pci_bus().config_space();
        let mut pci_device = pci_device.write();

        pci_device.update_command(pci_config_space, |command| {
            command.bitor(CommandRegister::BUS_MASTER_ENABLE | CommandRegister::MEMORY_ENABLE)
        });

        let bar0 = pci_device.bar(0, pci_config_space).expect("Failed to read BAR0");
        let base_address = bar0.unwrap_io() as u16;
        info!("Virtio Gpu Base address: {:#X}", base_address);

        let interrupt = InterruptVector::try_from(pci_device.interrupt(pci_config_space).1 +32).unwrap();

        let virtio_caps = [PciCapability::new(base_address); 16];
        info!("Virtio Caps: {:?}", virtio_caps);

        VirtioGpu {
            pci_device: 0,
            cap_ptr: 0,
            irq: 0,
            virtio_caps: [PciCapability::default(); 16],
            common_cfg: None,
            common_len: 0,
            isr: None,
            notify_ptr: None,
            notify_off_multiplier: 0,
            config: None,
            config_len: 0,
            rect: None,
            frame_buffer: None,
            queue_buffer_send: Box::new([]),
            queue_buffer_recv: Box::new([]),
        }
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