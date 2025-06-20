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
        //let mac = ne2k.read_mac();
        // ensure, that only one thread has access
        //info!(
        //    "NE2000 MAC address: [{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}]",
        //    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        //);
        // initial value which will be stored in Once
        //ne2k.clone()
        /*scheduler().ready(Thread::new_kernel_thread(|| loop {
                poll_ne2000();
            }, "Ne2K"));
        }*/

        //let mut ne2000 = Ne2000::new(devices2[0]);
        //ne2000.init();
        //let mac = ne2000.read_mac();
        //info!("8029 MAC address: [{}]", ne2000.read_mac());
        //info!(
        //    "NE2000 MAC address: [{:02X}-{:02X}-{:02X}-{:02X}-{:02X}-{:02X}]",
        //    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
        //);
        //});
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
    let ne = NE2000.get().unwrap();
    let mut sockets = SOCKETS.get().unwrap().write();
    // interface is connection between smoltcp crate and driver
    //let mut interface = ;

    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let dev = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };
    let time = Instant::from_millis(timer().systime_ms() as i64);

    //interface.poll(time, dev, &mut *sockets).unwrap();
}
