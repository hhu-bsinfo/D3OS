use crate::device::rtl8139::Rtl8139;
// add the N2000 driver
use crate::device::ne2k::{ne2000::Ne2000, network_stack::Ne2000TxToken};
use crate::process::thread::Thread;
use crate::{pci_bus, scheduler, timer};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};
use log::info;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp;
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
use spin::{Mutex, Once, RwLock};

static RTL8139: Once<Arc<Rtl8139>> = Once::new();
// ensure that the driver is only initialized once
static NE2000: Once<Arc<Ne2000>> = Once::new();

static INTERFACES: RwLock<Vec<Interface>> = RwLock::new(Vec::new());
static SOCKETS: Once<RwLock<SocketSet>> = Once::new();

pub enum SocketType {
    Udp,
}

pub fn init() {
    SOCKETS.call_once(|| RwLock::new(SocketSet::new(Vec::new())));

    let enable_rtl8139 = false;
    let enable_ne2k = true;

    if enable_rtl8139 {
        let devices = pci_bus().search_by_ids(0x10ec, 0x8139);
        if devices.len() > 0 {
            RTL8139.call_once(|| {
                info!("Found Realtek RTL8139 network controller");
                let rtl8139 = Arc::new(Rtl8139::new(devices[0]));
                info!("RTL8139 MAC address: [{}]", rtl8139.read_mac_address());

                Rtl8139::plugin(Arc::clone(&rtl8139));
                rtl8139
            });
        }

        if RTL8139.get().is_some() {
            scheduler().ready(Thread::new_kernel_thread(
                || loop {
                    poll_sockets();
                },
                "RTL8139",
            ));
        }
    }

    // Register the Ne2000 card here
    // wrap into Arc for shared ownership
    // Scans PCI bus for Ne2000 cards or similar by looking at the device id and vendor id.
    // References:
    //             - https://en.wikibooks.org/wiki/QEMU/Devices/Network -> the nic model
    //             - https://theretroweb.com/chips/4692 -> device id and vendor id
    if enable_ne2k {
        let device_ne2k = pci_bus().search_by_ids(0x10ec, 0x8029);
        if device_ne2k.len() > 0 {
            NE2000.call_once(|| {
                info!("\x1b[1;31mFound Realtek 8029 network controller");
                //let ne2k = Arc::new(Ne2000::new(devices2[0]));
                let device = Ne2000::new(device_ne2k[0]);
                let ne2k = Arc::new(device);

                //read the mac address
                info!("\x1b[1;31mNe2000 MAC address: [{}]", ne2k.read_mac());
                //enable interrupt handler
                Ne2000::assign(Arc::clone(&ne2k));
                info!("assigned Interrupt handler");
                ne2k
            });
        }

        // if NE2000 is initialized, start a new thread,
        // which calls poll_ne2000 in an infinite loop
        // the method checks for any outgoing or incoming packages in the buffers of
        // the device or in the buffers of let ne2k = Arc::new(Mutex::new(Ne2000::new(devices2[0])));the sockets
        if NE2000.get().is_some() {
            scheduler().ready(Thread::new_kernel_thread(
                || loop {
                    poll_ne2000();
                },
                "NE2000",
            ));
        }
    }
}

pub fn rtl8139() -> Option<Arc<Rtl8139>> {
    match RTL8139.get() {
        Some(rtl8139) => Some(Arc::clone(rtl8139)),
        None => None,
    }
}

// add ne2000 function
// safely share access to global, reference-counted nic
// Once get method : Returns a reference to the inner value if the Once has been initialized.
// Pattern matching : if some, return the cloned pointer of Ne2000
// else none
pub fn ne2000() -> Option<Arc<Ne2000>> {
    match NE2000.get() {
        Some(ne2000) => Some(Arc::clone(ne2000)),
        None => None,
    }
}

pub fn add_interface(interface: Interface) {
    INTERFACES.write().push(interface);
}

pub fn open_socket(protocol: SocketType) -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    // changed transmit and receive buffer size to tx_size and rx_size
    let tx_size = 1000;
    let rx_size = 1000;

    let rx_buffer = udp::PacketBuffer::new(
        // packetgröße auf 10 erhöhen
        vec![udp::PacketMetadata::EMPTY; rx_size],
        vec![0; 65535],
    );
    let tx_buffer =
        udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; tx_size], vec![0; 65535]);

    let socket = match protocol {
        SocketType::Udp => udp::Socket::new(rx_buffer, tx_buffer),
    };

    sockets.write().add(socket)
}

pub fn close_socket(handle: SocketHandle) {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    sockets.write().remove(handle);
}

pub fn bind_udp(handle: SocketHandle, port: u16) -> Result<(), udp::BindError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<udp::Socket>(handle);

    socket.bind(port)
}

pub fn send_datagram(
    handle: SocketHandle,
    destination: Ipv4Address,
    port: u16,
    data: &[u8],
) -> Result<(), udp::SendError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<udp::Socket>(handle);

    socket.send_slice(data, (destination, port))
}

// disabled for the rtl8139.rs
fn poll_sockets() {
    let rtl8139 = RTL8139.get().expect("RTL8139 not initialized");
    let mut interfaces = INTERFACES.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let time = Instant::from_millis(timer().systime_ms() as i64);

    // Smoltcp expects a mutable reference to the device, but the RTL8139 driver is built
    // to work with a shared reference. We can safely cast the shared reference to a mutable.
    let device = unsafe { ptr::from_ref(rtl8139.deref()).cast_mut().as_mut().unwrap() };

    for interface in interfaces.iter_mut() {
        interface.poll(time, device, &mut sockets);
    }
}

// poll for ne2k

fn poll_ne2000() {
    // interface is connection between smoltcp crate and driver
    // interfaces stores a Vector off all added Network Interfaces
    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000
        .get()
        .expect("[poll_ne2000] : Ne2000 not initialized.");
    let dev_ne2k = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };

    // acquire read lock
    /*let interfaces = INTERFACES.read();
    for (i, iface) in interfaces.iter().enumerate() {
        let s = iface.to_string();
        info!("{:?} {}", s, i);
    }*/

    // initialize Interfaces and sockets
    let mut interfaces = INTERFACES.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized").write();

    // start interface
    //let mut counter: u8 = 0;
    // iterate through every interface and call the poll method
    for iface in interfaces.iter_mut() {
        //info!("Polling, Iteration: {}", counter);
        // check if smoltcp processes something
        // poll calls the receive and transmit methods of the device impl in networks_stack/mod.rs
        // checking if any packets have been send or received
        iface.poll(now, dev_ne2k, &mut sockets);
    }
}
