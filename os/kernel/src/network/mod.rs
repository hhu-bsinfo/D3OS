use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use log::info;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp;
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
use spin::{Once, RwLock};
use crate::device::rtl8139::Rtl8139;
use crate::{pci_bus, scheduler, timer};
use crate::process::thread::Thread;

static RTL8139: Once<Rtl8139> = Once::new();

static INTERFACES: RwLock<Vec<Interface>> = RwLock::new(Vec::new());
static SOCKETS: Once<RwLock<SocketSet>> = Once::new();

pub enum SocketType {
    Udp
}

pub fn init() {
    SOCKETS.call_once(|| RwLock::new(SocketSet::new(Vec::new())));

    let devices = pci_bus().search_by_ids(0x10ec, 0x8139);
    if devices.len() > 0 {
        RTL8139.call_once(|| {
            info!("Found Realtek RTL8139 network controller");
            let rtl8139 = Rtl8139::new(devices[0]);
            info!("RTL8139 MAC address: [{}]", rtl8139.read_mac_address());

            rtl8139.plugin();
            return rtl8139;
        });
    }

    if RTL8139.get().is_some() {
        scheduler().ready(Thread::new_kernel_thread(|| loop {
            poll_sockets();
        }));
    }
}

pub fn rtl8139() -> Option<&'static Rtl8139> {
    RTL8139.get()
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

pub fn send_datagram(handle: SocketHandle, destination: Ipv4Address, port: u16, data: &[u8]) -> Result<(), udp::SendError> {
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
    let device = unsafe { ptr::from_ref(rtl8139).cast_mut().as_mut().unwrap() };

    for interface in interfaces.iter_mut() {
        interface.poll(time, device, &mut sockets);
    }
}