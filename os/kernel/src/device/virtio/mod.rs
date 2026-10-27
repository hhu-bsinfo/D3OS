use core::sync::atomic::AtomicBool;

use alloc::boxed::Box;
use log::{error, info, warn};
use spin::{Mutex, Once};
use virtio::{device::{gpu::VirtIOGpu, input::VirtIOInput, rng::VirtIORng, socket::VirtIOSocket, sound::VirtIOSound}, transport::{Transport, pci::{PciTransport, bus::{BarInfo, ConfigurationAccess, DeviceFunction, PciRoot}}}};
use x86_64::{PhysAddr, structures::paging::PageTableFlags};

use crate::{apic, interrupt::interrupt_dispatcher::InterruptVector, interrupt_dispatcher, memory::{PAGE_SIZE, vma::VmaType}, pci_bus, process_manager};
use interrupt::VirtioInterruptHandler;
use hal::HalImpl;
#[cfg(feature = "virtio_tests")]
use demo::{pong::pong_demo, rectangle::rectangle_demo, virtio_tests};

#[cfg(feature = "virtio_tests")]
mod demo;
mod dma;
mod hal;
mod interrupt;

static VIRTIO_RNG: Once<Mutex<VirtIORng<HalImpl, PciTransport>>> = Once::new();

static VIRTIO_GPU: Once<Mutex<VirtIOGpu<HalImpl, PciTransport>>> = Once::new(); //Arc hinzufügen? Mutex pflicht
pub static GPU_QUEUE_PENDING:  AtomicBool = AtomicBool::new(false);
pub static GPU_CONFIG_PENDING: AtomicBool = AtomicBool::new(false);

static VIRTIO_INPUT: Once<Mutex<VirtIOInput<HalImpl, PciTransport>>> = Once::new();
pub static VIRTIO_INPUT_PENDING: AtomicBool = AtomicBool::new(false);

static VIRTIO_SOCKET: Once<Mutex<VirtIOSocket<HalImpl, PciTransport>>> = Once::new();

static VIRTIO_SOUND: Once<Mutex<VirtIOSound<HalImpl, PciTransport>>> = Once::new();

pub fn virtio_rng() -> Option<&'static Mutex<VirtIORng<HalImpl, PciTransport>>> {
    VIRTIO_RNG.get()
}

pub fn virtio_input() -> Option<&'static Mutex<VirtIOInput<HalImpl, PciTransport>>> {
    VIRTIO_INPUT.get()
}

pub fn virtio_gpu() -> Option<&'static Mutex<VirtIOGpu<HalImpl, PciTransport>>> {
    VIRTIO_GPU.get()
}

pub fn virtio_socket() -> Option<&'static Mutex<VirtIOSocket<HalImpl, PciTransport>>> {
    VIRTIO_SOCKET.get()
}

pub fn virtio_sound() -> Option<&'static Mutex<VirtIOSound<HalImpl, PciTransport>>> {
    VIRTIO_SOUND.get()
}

/// Find and initialize virtio devices
pub fn init_devices(fb_start_phys_addr: u64, fb_end_phys_addr: u64) {
    info!("Searching for VirtIO devices...");
    let pci_bus = pci_bus();
    let pci_config_space = pci_bus.config_space();
    const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

    let virtio_devices = pci_bus.search_by_vendor(VIRTIO_VENDOR_ID);
    let mut pci_root = PciRoot::new(unsafe { pci_config_space.unsafe_clone() });

    if virtio_devices.is_empty() {
        info!("No VirtIO devices found on the PCI bus.");
    } else {
        
        for device_lock in virtio_devices.iter(){

            let address = device_lock.read().header().address();

            let device_function = DeviceFunction {
                bus: address.bus(),
                device: address.device(),
                function: address.function(),
            };

            for bar_index in 0..6 {
            if let Ok(Some(BarInfo::Memory { address, size, .. })) =
                pci_root.bar_info(device_function, bar_index)
            {
                if size == 0 {
                    continue;
                }

                if address % PAGE_SIZE as u64 != 0 {
                    warn!("      Skipping non-page-aligned 64-Bit Bar{} at {:#x}", bar_index, address);
                    continue;
                }

                let bar_start = address;
                let bar_end   = address + size;

                if overlaps(bar_start, bar_end, fb_start_phys_addr, fb_end_phys_addr) {
                    // Bereits gemappte Multiboot-LFB (virtio-vga BAR0) - Reuse
                    info!(
                        "      BAR{} {:#x}..{:#x} overlaps framebuffer {:#x}..{:#x} -> reusing existing mapping 'framebuffer'",
                        bar_index, bar_start, bar_end, fb_start_phys_addr, fb_end_phys_addr
                    );
                    let fb_end_aligned = PhysAddr::new(fb_end_phys_addr).align_up(PAGE_SIZE as u64).as_u64();

                    if bar_end > fb_end_aligned {
                        let tail_start = fb_end_aligned;
                        let tail_end = bar_end;
                        
                        let tail_size = tail_end - tail_start; // Für Terminalausgabe

                        info!("      Mapping BAR{} tail at {:#x} (size: {:#x})", bar_index, tail_start, tail_size);
                        
                        let kernel_process = process_manager().read().kernel_process().unwrap();
                        kernel_process.virtual_address_space.kernel_map_devm_identity(
                            tail_start,
                            tail_end,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
                            VmaType::DeviceMemory,
                            "virtio-gpu-vram-tail",
                        );
                    }
                    continue;
                }

                // Kein Overlap - normal identitätsmappen
                if address == 0 {
                    warn!("      Not mapping BAR{} at {:#x} (size: {:#x})", bar_index, address, size);
                    continue;
                }
                info!("      Mapping BAR{} at {:#x} (size: {:#x})", bar_index, address, size);
                let kernel_process = process_manager().read().kernel_process().unwrap();
                kernel_process.virtual_address_space.kernel_map_devm_identity(
                    address,
                    address + size,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
                    VmaType::DeviceMemory,
                    "virtio-bar",
                );
            }
        }

        if let Some(mut device_writer) = device_lock.try_write() {

            device_writer.update_command(pci_bus.config_space(), |cmd| {
                cmd | pci_types::CommandRegister::BUS_MASTER_ENABLE
                    | pci_types::CommandRegister::MEMORY_ENABLE
                    | pci_types::CommandRegister::IO_ENABLE
                    //| pci_types::CommandRegister::INTERRUPT_DISABLE
            });

            let (_, interrupt_line) = device_writer.interrupt(pci_bus.config_space());
            if interrupt_line != 0 && interrupt_line != 0xFF {
                let interrupt_vector = InterruptVector::try_from(interrupt_line + 32).unwrap();
                info!("    VirtIO device uses interrupt vector {:?}", interrupt_vector);
                interrupt_dispatcher().assign(interrupt_vector, Box::new(VirtioInterruptHandler));
                apic().allow(interrupt_vector);
            }

        } else {
            warn!("Konnte keinen Schreibzugriff auf VirtIO-Gerät erhalten, wird übersprungen.");
            return;
        }

            match PciTransport::new::<HalImpl, _>(&mut pci_root, device_function) {
                Ok(transport) => {
                    match transport.device_type() {
                        virtio::transport::DeviceType::GPU => {
                            info!("     VirtIO GPU device found. Initializing driver...");
                            VIRTIO_GPU.call_once(|| {
                                Mutex::new(
                                    VirtIOGpu::<HalImpl, PciTransport>::new(transport)
                                        .expect("Failed to create VirtIO GPU driver")
                                )
                            });
                        }
                        virtio::transport::DeviceType::EntropySource => {
                            info!("     VirtIO RNG device found. Initializing driver...");
                            VIRTIO_RNG.call_once(|| {
                                Mutex::new(
                                    VirtIORng::<HalImpl, PciTransport>::new(transport)
                                        .expect("Failed to create VirtIO Rng driver")
                                )
                            });
                        }
                        virtio::transport::DeviceType::Input => {
                            info!("     VirtIO Input device found. Initializing driver...");
                            VIRTIO_INPUT.call_once(|| {
                                Mutex::new(
                                    VirtIOInput::<HalImpl, PciTransport>::new(transport)
                                        .expect("Failed to create VirtIO Input driver")
                                )
                            });
                        }
                        virtio::transport::DeviceType::Socket => {
                            info!("     VirtIO Socket device found. Initializing driver...");
                            VIRTIO_SOCKET.call_once(|| {
                                Mutex::new(
                                    VirtIOSocket::<HalImpl, PciTransport>::new(transport)
                                        .expect("Failed to create VirtIO Socket driver")
                                )
                            });
                        }
                        virtio::transport::DeviceType::Sound => {
                            info!("     VirtIO Sound device found. Initializing driver...");
                            VIRTIO_SOUND.call_once(|| {
                                Mutex::new(
                                    VirtIOSound::<HalImpl, PciTransport>::new(transport)
                                        .expect("Failed to create VirtIO Sound driver")
                                )
                            });
                        }
                        dt => {
                            warn!("Unbehandelter Typ: {:?}", dt);
                        }
                    }
                }
                Err(e) => {
                    error!("Fehler: {:?}", e);
                }
            }
        }
    }
}

#[cfg(feature = "virtio_tests")]
pub fn run_tests() {
    virtio_tests::test_rng();
    virtio_tests::play_pcm_file();
    virtio_tests::test_virgl();
    if let Some(gpu_mutex) = virtio_gpu() {
        pong_demo(gpu_mutex);
        rectangle_demo(gpu_mutex);
    } else {
        warn!("VirtIO GPU nicht gefunden, Demo wird übersprungen.");
    }
}

#[inline(always)]
const fn overlaps(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    // halb-offene Intervalle [start, end)
    !(a_end <= b_start || b_end <= a_start)
}
