use crate::memory;

use super::dma::Dma;
use core::ptr::NonNull;
use virtio::{BufferDirection, Hal, PhysAddr};
use x86_64::structures::paging::PhysFrame;

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    /// Alloziert physisch zusammenhängende Speicherseiten für DMA.
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let dma_buffer = Dma::new(pages);

        let paddr = dma_buffer.paddr().as_u64() as PhysAddr;
        let vaddr = dma_buffer.vaddr(0);
        
        // Speicher nur per dealloc vergebbar
        core::mem::forget(dma_buffer);

        (paddr, vaddr)
    }

    /// Dealloziert die zuvor mit `dma_alloc` allozierten Speicherseiten.
    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let start_frame =
            PhysFrame::from_start_address(x86_64::PhysAddr::new(paddr as u64)).unwrap();
        let frame_range = x86_64::structures::paging::frame::PhysFrameRange {
            start: start_frame,
            end: start_frame + pages as u64,
        };
        memory::free_frames(frame_range);
        0
    }

    /// Wandelt eine physische MMIO-Adresse in eine virtuelle Adresse um.
    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as *mut u8).unwrap()
    }

    /// Gibt einen Speicherbereich für das Gerät frei und gibt die physische Adresse zurück.
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        buffer.as_ptr() as *mut u8 as PhysAddr
    }

    /// Beendet die Freigabe eines Speicherbereichs für das Gerät.
    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nichts tun
    }
}

// Drop-Implementierung
impl Drop for Dma {
    fn drop(&mut self) {
        let start_frame = PhysFrame::from_start_address(self.paddr()).unwrap();
        let frame_range = x86_64::structures::paging::frame::PhysFrameRange {
            start: start_frame,
            end: start_frame + self.pages() as u64,
        };
        memory::free_frames(frame_range);
    }
}