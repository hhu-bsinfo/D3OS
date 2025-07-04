// =============================================================================
// FILE        : network_stack/mod.rs
// AUTHOR      : Johann Spenrath
// DESCRIPTION : file includes the network stack for the NE2000 driver
// =============================================================================
//
//
//
// TODO:
//
// NOTES:
//
// =============================================================================
//
//
//
//
//
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
use crate::{apic, device, interrupt_dispatcher, pci_bus, process_manager, scheduler};
use core::{ptr, slice};
use log::info;
// for allocator impl
use core::alloc::{AllocError, Allocator, Layout};
// for allocator impl
use core::ptr::NonNull;

// lock free algorithms and datastructes
// queues: different queue implementations
// mpsc : has the jiffy queue ; lock-free unbounded
use nolock::queues::{mpmc, mpsc};

use pci_types::{CommandRegister, EndpointHeader};
// smoltcp provides a full network stack for creating packets, sending, receiving etc.
use alloc::vec::Vec;
use smoltcp::phy;
use smoltcp::phy::{DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use spin::{Mutex, RwLock};

// for writing to the registers
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};

use super::ne2000::*;
pub struct Ne2000TxToken<'a> {
    device: &'a mut Ne2000,
}

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
        //let mut buffer = [0u8; 1514];
        //let data = &mut buffer[..len];
        //let result = f(data);

        // call send method using the NE2000
        // TODO: implement send Methode
        //self.device.send_packet(data);
        //info!("Don't leave me here");
        let phys_buffer = frames::alloc(1);
        let phys_start_addr = phys_buffer.start.start_address();
        let pages = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_start_addr.as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_buffer.end.start_address().as_u64()))
                .unwrap(),
        };

        let kernel_process = process_manager().read().kernel_process().unwrap();
        kernel_process.virtual_address_space.set_flags(
            pages,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
        );

        // Queue physical memory buffer for deallocation after transmission (.enqueue)
        //.1 is the Sender here
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
        let result = f(buffer);

        // Send packet by writing physical address and packet length to transmit registers
        self.device.send_packet(buffer);

        result
    }
}

// allocate blocks of data
#[derive(Default)]
pub struct PacketAllocator;

unsafe impl Allocator for PacketAllocator {
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        panic!("PacketAllocator does not support allocate!");
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != PAGE_SIZE {
            panic!("PacketAllocator may only be used with page frames!");
        }

        let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64))
            .expect("PacketAllocator may only be used with page frames!");
        unsafe {
            frames::free(PhysFrameRange {
                start,
                end: start + 1,
            })
        }
    }
}

//for the moment not implemented
pub struct Ne2000RxToken<'a> {
    buffer: Vec<u8, PacketAllocator>,
    device: &'a Ne2000,
}

impl<'a> phy::RxToken for Ne2000RxToken<'a> {
    fn consume<R, F>(mut self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let result = f(&mut self.buffer);
        self.device
            .recv_buffers_empty
            .1
            .try_enqueue(self.buffer)
            .expect("Failed to enqueue used receive buffer!");

        result
        // Return empty slice
        //f(&[])
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

    // called by smoltcp, when polling for new packets
    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        // disable receive for now; only transmit exists
        //self.transmit(_timestamp).map(|tx| (Ne2000RxToken, tx))
        None
    }

    // Converts &mut self to &Ne2000 safely.
    // Needed because RxToken and TxToken store a shared reference to the driver (not &mut self). See RTL8139 impl
    // Returns a TxToken, which accepts the packet contents
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        //let device = unsafe { ptr::from_ref(self).as_ref()? };
        info!("==> transmit() requested by smoltcp!");
        Some(Ne2000TxToken::new(self))
    }

    // define what the device supports
    //max_burst_size = only send one packet at a time
    // medium = send packet over Ethernet
    // max_transmission_unit = define max. size of a packet
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = Some(1);
        caps.medium = Medium::Ethernet;

        caps
    }
}
