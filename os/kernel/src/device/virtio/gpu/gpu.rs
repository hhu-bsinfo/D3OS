use alloc::boxed::Box;
use alloc::sync::Arc;
use crate::pci_bus;
use log::info;
use pci_types::EndpointHeader;
use spin::{Mutex, RwLock};
use uefi::proto::console::gop::FrameBuffer;
use x86_64::instructions::port::{Port, PortReadOnly};
use crate::device::rtl8139::Command;
use crate::device::virtio::transport::capabilities::CommonCfg;
use crate::device::virtio::transport::flags::DeviceStatusFlags;

const VIRTIO_GPU_MAX_SCANOUTS: usize = 16;

#repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtioGpu {
    pciDevice: RwLock<EndpointHeader>,
    rect: Option<VirtioGpuRect>,
    //frame_buffer: FrameBuffer
    // cursot_buffer not implemented
    //control_queue: VirtioQueue,
    queue_buffer_send: Box<[u8]>,
    queue_buffer_recv: Box<[u8]>,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct GpuConfig {
    /// signals pending events to the driver. The driver MUST NOT write to this field.
    pub events_read: u32,
    /// clears pending events in the device. Writing a ’1’ into a bit will clear the corresponding bit in events_read, mimicking write-to-clear behavior.
    pub events_clear: u32,
    /// specifies the maximum number of scanouts supported by the device. Minimum value is 1, maximum value is 16.
    pub num_scanouts: u32,
    /// specifies the maximum number of capability sets supported by the device. The minimum value is zero.
    pub num_capsets: u32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum VirtioGpuCtrlType {
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
#[derive(Debug, Clone, Copy)]
pub struct VirtioGpuCtrlHdr {
    pub type_: VirtioGpuCtrlType, // Safe to use
    pub flags: u32,
    pub fence_id: u64,
    pub ctx_id: u32,
    pub ring_idx: u8,
    pub padding: [u8; 3], // Explicit padding for struct alignment
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioGpuRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl VirtioGpuRect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioGpuDisplayOne {
    pub r: VirtioGpuRect,
    pub enabled: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioGpuRespDisplayInfo {
    pub hdr: VirtioGpuCtrlHdr,
    pub pmodes: [VirtioGpuDisplayOne; VIRTIO_GPU_MAX_SCANOUTS],
}

impl Default for VirtioGpuRespDisplayInfo {
    fn default() -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: VirtioGpuCtrlType::GetDisplayInfo,
                ..Default::default()
            },

            pmodes: unsafe { core::mem::zeroed() },
        }
    }
}


pub struct VirtioGpuDevice {
    device: Arc<Mutex<EndpointHeader>>,
    io_base: u16,
    irq: u8,
}

impl VirtioGpuDevice {
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
}

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