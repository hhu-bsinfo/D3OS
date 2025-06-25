use crate::device::rtl8139::Rtl8139;
// add the N2000 driver
use crate::device::ne2000::Ne2000;
use crate::process::thread::Thread;
use crate::{pci_bus, scheduler, timer};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use log::info;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp;
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
use spin::{Mutex, Once, RwLock};

use smoltcp::socket::udp::{PacketBuffer, PacketMetadata, Socket};
use smoltcp::wire::{IpAddress, IpEndpoint};

static RTL8139: Once<Arc<Rtl8139>> = Once::new();
// ensure that the driver is only initialized once
//static NE2000: Once<Arc<Mutex<Ne2000>>> = Once::new();
static NE2000: Once<Arc<Ne2000>> = Once::new();

static INTERFACES: RwLock<Vec<Interface>> = RwLock::new(Vec::new());
static SOCKETS: Once<RwLock<SocketSet>> = Once::new();

pub enum SocketType {
    Udp,
}

pub fn init() {
    SOCKETS.call_once(|| RwLock::new(SocketSet::new(Vec::new())));

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

    // TODO: Implement NE2000.rs
    // Register the Ne2000 card here
    // wrap into Arc for shared ownership
    // Scans PCI bus for Ne2000 cards or similar by looking at the device id and vendor id.
    // TODO: add reference for vendor and device id here
    let devices2 = pci_bus().search_by_ids(0x10ec, 0x8029);
    if devices2.len() > 0 {
        NE2000.call_once(|| {
            info!("Found Realtek 8029 network controller");
            let ne2k = Arc::new(Ne2000::new(devices2[0]));

            info!("Ne2000 MAC address: [{}]", ne2k.read_mac());
            ne2k
        });
    }

    // if NE2000 is initialized, start a new thread,
    // which calls poll_ne2000 in an infinite loop
    // the method checks for any outgoing or incoming packages in the buffers of
    // the device or in the buffers of the sockets
    if NE2000.get().is_some() {
        scheduler().ready(Thread::new_kernel_thread(
            || loop {
                poll_ne2000();
            },
            "NE2000",
        ));
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

    let rx_buffer = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );
    let tx_buffer = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );

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
    info!("i hope this works");
    // interface is connection between smoltcp crate and driver
    // interfaces stores a Vector off all added Network Interfaces
    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000.get().unwrap();
    let dev_ne2k = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };

    let mut sockets = SOCKETS.get().unwrap().write();

    // Crate UDP socket with buffers
    let rx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY], vec![0; 512]);
    let tx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY], vec![0; 512]);
    let socket = Socket::new(rx_buffer, tx_buffer);
    let handle = sockets.add(socket);

    // Bind, enqueue packet
    let mut sock = sockets.get_mut::<Socket>(handle);
    sock.bind(1234).unwrap();

    let destination = IpEndpoint::new(IpAddress::v4(10, 0, 2, 2), 5678);
    sock.send_slice(b"i hope this works", destination).unwrap();

    // start interface
    let mut interfaces = INTERFACES.write();
    for iface in interfaces.iter_mut() {
        // This will call your NE2000 TxToken and perform the send
        info!("i hope this works");
        iface.poll(now, dev_ne2k, &mut sockets);
    }
}
