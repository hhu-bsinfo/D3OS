use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use log::info;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::{icmp, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::IpAddress;
use spin::{Once, RwLock};
use crate::device::rtl8139::Rtl8139;
use crate::{pci_bus, scheduler, timer};
use crate::process::thread::Thread;

static RTL8139: Once<Arc<Rtl8139>> = Once::new();

static INTERFACES: RwLock<Vec<Interface>> = RwLock::new(Vec::new());
static SOCKETS: Once<RwLock<SocketSet>> = Once::new();

#[derive(Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum SocketType {
    Udp, Tcp, Icmp,
}

pub fn init() {
    SOCKETS.call_once(|| RwLock::new(SocketSet::new(Vec::new())));

    let devices = pci_bus().search_by_ids(0x10ec, 0x8139);
    if !devices.is_empty() {
        RTL8139.call_once(|| {
            info!("Found Realtek RTL8139 network controller");
            let rtl8139 = Arc::new(Rtl8139::new(devices[0]));
            info!("RTL8139 MAC address: [{}]", rtl8139.read_mac_address());

            Rtl8139::plugin(Arc::clone(&rtl8139));
            rtl8139
        });
    }

    if RTL8139.get().is_some() {
        extern "sysv64" fn poll() {
            loop { poll_sockets(); }
        }
        scheduler().ready(Thread::new_kernel_thread(poll, "RTL8139"));
    }
}

pub fn rtl8139() -> Option<Arc<Rtl8139>> {
    match RTL8139.get() {
        Some(rtl8139) => Some(Arc::clone(rtl8139)),
        None => None
    }
}

pub fn add_interface(interface: Interface) {
    INTERFACES.write().push(interface);
}

pub fn open_udp() -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");

    let rx_buffer = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );
    let tx_buffer = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );

    sockets.write().add(udp::Socket::new(rx_buffer, tx_buffer))
}

pub fn open_tcp() -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    let rx_buffer = tcp::SocketBuffer::new(vec![0; 65535]);
    let tx_buffer = tcp::SocketBuffer::new(vec![0; 65535]);

    sockets.write().add(tcp::Socket::new(rx_buffer, tx_buffer))
}

pub fn open_icmp() -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    
    let rx_buffer = icmp::PacketBuffer::new(
        vec![icmp::PacketMetadata::EMPTY, icmp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );
    let tx_buffer = icmp::PacketBuffer::new(
        vec![icmp::PacketMetadata::EMPTY, icmp::PacketMetadata::EMPTY],
        vec![0; 65535],
    );

    sockets.write().add(icmp::Socket::new(rx_buffer, tx_buffer))
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

pub fn bind_tcp(handle: SocketHandle, port: u16) -> Result<(), tcp::ListenError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<tcp::Socket>(handle);

    socket.listen(port)
}

pub fn bind_icmp(handle: SocketHandle, ident: u16) -> Result<(), icmp::BindError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<icmp::Socket>(handle);

    socket.bind(icmp::Endpoint::Ident(ident))
}

pub fn accept_tcp(handle: SocketHandle) -> Result<u16, tcp::ConnectError> {
    // TODO: smoltcp knows no backlog
    // all but the first connection will fail
    loop {
        // this extra block is needed so that we don't block all sockets
        {
            let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
            let socket = sockets.get_mut::<tcp::Socket>(handle);
            
            if socket.is_active() {
                break;
            }
        }
        scheduler().sleep(100);
    }
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<tcp::Socket>(handle);
    
    // TODO: pass the remote addr
    Ok(socket.remote_endpoint().unwrap().port)
}

pub fn connect_tcp(handle: SocketHandle, host: IpAddress, port: u16) -> Result<u16, tcp::ConnectError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let mut interfaces = INTERFACES.write();

    let socket = sockets.get_mut::<tcp::Socket>(handle);

    let interface = interfaces.get_mut(0).ok_or(tcp::ConnectError::InvalidState)?;
    let local_port = 1797; // TODO

    socket.connect(interface.context(), (host, port), local_port)?;
    // TODO: pass the local addr
    Ok(socket.local_endpoint().unwrap().port)
}

pub fn send_datagram(handle: SocketHandle, destination: IpAddress, port: u16, data: &[u8]) -> Result<(), udp::SendError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<udp::Socket>(handle);

    socket.send_slice(data, (destination, port))
}

pub fn send_tcp(handle: SocketHandle, data: &[u8]) -> Result<usize, tcp::SendError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<tcp::Socket>(handle);

    socket.send_slice(data)
}

pub fn send_icmp(handle: SocketHandle, destination: IpAddress, data: &[u8]) -> Result<(), icmp::SendError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<icmp::Socket>(handle);

    socket.send_slice(data, destination)
}

pub fn receive_datagram(handle: SocketHandle, data: &mut [u8]) -> Result<(usize, udp::UdpMetadata), udp::RecvError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<udp::Socket>(handle);

    socket.recv_slice(data)
}

pub fn receive_tcp(handle: SocketHandle, data: &mut [u8]) -> Result<usize, tcp::RecvError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<tcp::Socket>(handle);

    socket.recv_slice(data)
}

pub fn receive_icmp(handle: SocketHandle, data: &mut [u8]) -> Result<(usize, IpAddress), icmp::RecvError> {
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let socket = sockets.get_mut::<icmp::Socket>(handle);

    socket.recv_slice(data)
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