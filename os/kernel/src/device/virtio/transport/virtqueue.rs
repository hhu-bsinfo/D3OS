use alloc::vec;
use alloc::vec::Vec;

pub const VIRTQUEUE_SIZE: usize = 64;
pub const MAX_DESCRIPTORS: usize = 64;
pub const VIRTQ_DESC_F_NEXT: u16 = 1;
pub const VIRTQ_DESC_F_WRITE: u16 = 2;
pub const VIRTQ_DESC_F_INDIRECT: u16 = 4;


pub struct VirtioQueue {
    queue_size: u16,
    descriptors: Vec<Descriptor>,
    available: AvailableRing,
    used: UsedRing,
    last_used_idx: u16,
    last_avail_idx: u16,
    buffer: Vec<u16>,
    chunk_size: u32,
    next_buffer: u16,
    lock: u64,
}

// Virtual I/O Device (VIRTIO) Version 1.3, section 2.7.5: The Virtqueue Descriptor Table
#[repr(C)]
pub struct Descriptor {
    address: u64,
    length: u32,
    flags: DescriptorFlags,
    next: u16,
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    #[repr(transparent)]
    pub struct DescriptorFlags: u16 {
        // This marks a buffer as continuing via the next field.
        const VIRTQ_DESC_F_NEXT = 1 << 0;
        // This marks a buffer as device write-only (otherwise device read-only).
        const VIRTQ_DESC_F_WRITE = 1 << 1;
        // This means the buffer contains a list of buffer descriptors.
        const VIRTQ_DESC_F_INDIRECT = 1 << 2;
    }
}

// Virtual I/O Device (VIRTIO) Version 1.3, section 2.7.6: The Virtqueue Available Ring
// Also called Driver Ring. Driver to device communication
#[repr(C)]
pub struct AvailableRing {
    flags: u16,
    index: u16,
    ring: [u16; VIRTQUEUE_SIZE],
}

// Also called Device Ring. Device to driver communication
#[repr(C)]
pub struct UsedRing {
    flags: u16,
    index: u16,
    ring: [UsedRingElement; VIRTQUEUE_SIZE],
}

#[repr(C)]
pub struct UsedRingElement {
    id: u32,
    length: u32,
}

#[repr(C)]
pub struct DmaBuffer {
    pub address: u64,
    pub length: u32,
}

