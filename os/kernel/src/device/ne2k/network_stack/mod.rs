// =============================================================================
// FILE        : network_stack/mod.rs
// AUTHOR      : Johann Spenrath
// DESCRIPTION : file includes the network stack for the NE2000 driver
//               which is provided by the smoltcp crate
//               and implementations for Packet allocation and deallocation
// =============================================================================
//
// NOTES:
//
//
// =============================================================================
//
//
// =============================================================================
//& borrowing the Struct Ne2000

// 'a lifetime annotation
// implementation is orientated on the rtl8139.rs module

// changed to mut because send packet expects mutable self reference
//

use crate::memory::{PAGE_SIZE, frames};
use crate::process_manager;
use core::{ptr, slice};
use log::info;
// for allocator impl
use core::alloc::{AllocError, Allocator, Layout};
// for allocator impl
use core::ptr::NonNull;

// lock free algorithms and datastructes
// queues: different queue implementations
// mpsc : has the jiffy queue ; lock-free unbounded

// smoltcp provides a full network stack for creating packets, sending, receiving etc.
use alloc::vec::Vec;
use smoltcp::phy;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::time::Instant;

// for writing to the registers
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};

use super::ne2000::*;

// NIC uses two ring buffers for packet handling, which are made of 256 Byte Pages
// Reference: https://wiki.osdev.org/Ne2000#Ring_Buffer

// one page in the ne2000 receive buffer is 256 byte
const NE_PAGE_SIZE: usize = 256;
//Packet Header Size
const HEADER_SIZE: usize = 4;
// max. size of one Ethernet packet
const MAX_FRAME_SIZE: usize = 1500;
// max. Buffer size
const BUFFER_SIZE: usize = HEADER_SIZE + MAX_FRAME_SIZE; // = 1504
// calculate how many Pages in the NIC are needed to store a Packet with a MTU
const BUFFER_PAGES: usize = (BUFFER_SIZE + NE_PAGE_SIZE - 1) / NE_PAGE_SIZE; // = 6
// size of the actual buffer
const TOTAL_BUFFER_BYTES: usize = BUFFER_PAGES * NE_PAGE_SIZE; // = 1536

// => (16 KiB + 4 KiB -1)/4 KiB = 4 pages
//const FRAME_PAGES: usize = (BUFFER_RING_BYTES + PAGE_SIZE - 1) / PAGE_SIZE;

// size of each receive buffer
// 8 KiB = 8x1024
// + 16 bytes for header/alignment
// +1500 : max. Ethernet MTU payload size
// => 9708 bytes per buffer
// ensures each buffer can hold an ethernet frame payload + space for padding
//const BUFFER_SIZE: usize = 8 * 1024 + 16 + 1500;
// how many buffer pages are needed to cover buffer size
// 9708 / 4096 = 2.37 -> else part -> 3 Pages
// Page size is 4096 bytes
/*const BUFFER_PAGES: usize = if BUFFER_SIZE % PAGE_SIZE == 0 {
    BUFFER_SIZE / PAGE_SIZE
} else {
    BUFFER_SIZE / PAGE_SIZE + 1
};*/

// =============================================================================
// ==== STRUCTS
// ======|> pub struct Ne2000TxToken<'a>
// ======|> pub struct Ne2000RxToken<'a>
// ======|> pub struct PacketAllocator
// ======|> pub struct PacketAllocator;
// =============================================================================

// closure that, when run,
// writes whatever bytes are sent onto the link
pub struct Ne2000TxToken<'a> {
    device: &'a mut Ne2000,
}
// Receive Token for the driver, points to the
// ne2000 struct,
// tokens are types that allow to receive/send a single packet,
// receive and transmit construct the tokens only
// real sending, tranmitting is done by the consume
pub struct Ne2000RxToken<'a> {
    buffer: Vec<u8, PacketAllocator>,
    device: &'a Ne2000,
}

// allocate blocks of data
// Ne2000 uses buffer ring,
// packets can be overwritten by new incoming packets once
// the buffer is full
// driver allocates memory in RAM to copy the packet there
// and free the buffer on NE2000
#[derive(Default)]
pub struct PacketAllocator;

// =============================================================================
// ==== IMPLEMENTATIONS
// =============================================================================

// implementation is orientated on the rtl8139.rs module
// generate new transmission token
// a token to send a single network packet
// see: https://docs.rs/smoltcp/latest/smoltcp/phy/trait.TxToken.html

impl<'a> Ne2000TxToken<'a> {
    pub fn new(device: &'a mut Ne2000) -> Self {
        Self { device }
    }
}

// implementation is orientated on the rtl8139.rs module
// len: size of packet
impl<'a> phy::TxToken for Ne2000TxToken<'a> {
    // consumes the token to send a single network packet
    // constructs buffer (size len) -> calls passed closure f
    // in the closure a valid network packet should be constructed
    // when closure returns, packet gets send out
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        // Allocate and fill local buffer
        // max. buffer size is 1514 (see documentation )
        // TODO: add reference in manual for this

        // call send method using the NE2000
        // allocate one pyhsical frame
        // the phys_buffers gets a start and end PhysFrame (Range)
        // for defining where the packet gets written
        let phys_buffer = frames::alloc(1);
        let phys_start_addr = phys_buffer.start.start_address();
        // map to kernel space
        let pages = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_start_addr.as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_buffer.end.start_address().as_u64()))
                .unwrap(),
        };

        // set kernel page tables to writable, no_caching for DMA,
        // ensure buffer is present in memory
        //map it writable & uncached for DMA
        let kernel_process = process_manager().read().kernel_process().unwrap();
        kernel_process.virtual_address_space.set_flags(
            pages,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
        );

        // Queue physical memory buffer for deallocation after transmission (.enqueue)
        //.1 is the Sender here
        // nic then sends the packet over the network
        self.device
            .send_queue
            .1
            .enqueue(phys_buffer)
            .expect("Failed to enqueue physical buffer!");

        // Let smoltcp write the packet data to the buffer
        // slice : a view into a block of memory represented as a pointer and a length.
        // example:
        //let mut x = [1, 2, 3];
        //let x = &mut x[..]; // Take a full slice of `x`.
        //x[1] = 7;
        //assert_eq!(x, &[1, 7, 3]);
        // from_raw_parts_mut : Forms a mutable slice from a pointer and a length.

        let buffer = unsafe {
            slice::from_raw_parts_mut(phys_buffer.start.start_address().as_u64() as *mut u8, len)
        };
        // closure builds the ethernet frame
        let result = f(buffer);

        // Send packet
        // function sets the register, writes the packet per remote dma from the host to the local
        // buffer memory of the nic and triggers a send operation
        self.device.send_packet(buffer);

        result
    }
}

unsafe impl Allocator for PacketAllocator {
    // from rtl8139.rs
    // used for allocating memory for the RxToken Buffer
    // allocates a block of memory
    // returns NonNull, which meets the size and alignment of layout, remains
    // valid as long as it is currently allocated
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        panic!("PacketAllocator does not support allocate!");
    }

    // deallocate memory referenced by the pointer
    // return one page frame of physical memory back to allocator
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != PAGE_SIZE {
            panic!("PacketAllocator may only be used with page frames!");
        }

        // get the raw pointer, convert to u64 -> physical memory adress
        // PhysAddr wraps address in a PhysAddr type
        // contruct Frame from the Address, must be page-aligned, (divisible by page size )
        let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64))
            .expect("PacketAllocator may only be used with page frames!");
        unsafe {
            // create one physical page frage
            // frames::free -> return to memory allocator
            frames::free(PhysFrameRange {
                start,
                end: start + 1,
            })
        }
    }
}

impl<'a> Ne2000RxToken<'a> {
    pub fn new(buffer: Vec<u8, PacketAllocator>, device: &'a Ne2000) -> Self {
        Self { buffer, device }
    }
}

impl<'a> phy::RxToken for Ne2000RxToken<'a> {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        // buffer contains payload, which has been written to by the
        // receive_packages function
        // get the result by applying the closure to the buffer
        let result = f(&mut self.buffer);
        // enqueue the used allocated buffer to the receive_buffers_empty
        // queue, to use it again during packet reception
        self.device
            .receive_buffers_empty
            .1
            .try_enqueue(self.buffer)
            .expect("Failed to enqueue used receive buffer!");

        //return the result (the received data)
        result
    }
}

impl phy::Device for Ne2000 {
    type RxToken<'a>
        = Ne2000RxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = Ne2000TxToken<'a>
    where
        Self: 'a;

    // called by smoltcp, when polling for new packets in network/mod.rs in poll_ne2k
    // From https://docs.rs/smoltcp/latest/smoltcp/phy/trait.Device.html
    // The additional transmit token makes it possible to generate a reply packet
    // based on the contents of the received packet. For example, this makes it
    // possible to handle arbitrarily large ICMP echo (“ping”) requests,
    // where the all received bytes need to be sent back, without heap allocation.
    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let device = unsafe { ptr::from_ref(self).as_ref()? };
        //dequeue an empty buffer from receive_messages, which gets assigned
        // to the Ne2000RxToken for loading the packet payload
        // 0 is the receiver of the queue
        // if a packet is enqueued, it returns Ok(rec_buf)
        // and tries to pop it off the receive_messages queue
        // Ne2000RxToken – gives smoltcp read-only access to the just-received Ethernet frame.
        // new(recv_buf, device) ties the buffer together
        // with a reference to the NIC so that, when smoltcp
        // finishes with the frame, the token can safely return the buffer to the driver.
        match self.receive_messages.0.try_dequeue() {
            Ok(recv_buf) => Some((
                Ne2000RxToken::new(recv_buf, device),
                Ne2000TxToken::new(self),
            )),
            // no packet has been received and is waiting in the queue
            Err(_) => None,
        }
    }

    // Converts &mut self to &Ne2000 safely.
    // Needed because RxToken and TxToken store a shared reference to the driver (not &mut self). See RTL8139 impl
    // Returns a TxToken, which accepts the packet contents
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        //let device = unsafe { ptr::from_ref(self).as_ref()? };
        //info!("==> transmit() requested by smoltcp!");
        //if self.ready2transmit() {
        //    Some(Ne2000TxToken::new(self))
        //} else {
        //    None
        //}
        Some(Ne2000TxToken::new(self))
    }

    // define what the device supports
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        // max_transmission_unit = define max. size of a packet
        // this is the size of one ethernet frame
        // see: https://en.wikipedia.org/wiki/Ethernet_frame
        caps.max_transmission_unit = TOTAL_BUFFER_BYTES;
        //max_burst_size = only send one packet at a time
        //caps.max_burst_size = Some(1);
        caps.max_burst_size = None;
        // medium = send packet over Ethernet
        caps.medium = Medium::Ethernet;

        // return capabilities
        caps
    }
}
