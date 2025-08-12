use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use smoltcp::socket::dns::GetQueryResultError;
use core::net::{Ipv4Addr, Ipv6Addr};
use core::ops::Deref;
use core::ptr;
use log::{info, warn};
use smoltcp::iface::{self, Interface, SocketHandle, SocketSet};
use smoltcp::socket::{dhcpv4, dns, icmp, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{DnsQueryType, HardwareAddress, IpAddress, IpCidr, IpEndpoint};
use spin::{Once, RwLock};
use crate::device::rtl8139::Rtl8139;
use crate::process::process::Process;
use crate::{pci_bus, process_manager, scheduler, timer};
use crate::process::thread::Thread;

static RTL8139: Once<Arc<Rtl8139>> = Once::new();

static INTERFACES: RwLock<Vec<Interface>> = RwLock::new(Vec::new());
static SOCKETS: Once<RwLock<SocketSet>> = Once::new();
/// This maps sockets to the respective process.
/// We use this to check whether a process can access a particular socket.
/// We can't just create a SocketSet per process because smoltcp drops all
/// packets for non-existing sockets when polling.
static SOCKET_PROCESS: RwLock<BTreeMap<SocketHandle, Arc<Process>>> = RwLock::new(BTreeMap::new());
static DNS_SOCKET: Once<SocketHandle> = Once::new();
static DHCP_SOCKET: Once<SocketHandle> = Once::new();

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

    if let Some(rtl8139) = RTL8139.get() {
        extern "sysv64" fn poll() {
            loop { poll_sockets(); scheduler().sleep(50); }
        }
        scheduler().ready(Thread::new_kernel_thread(poll, "RTL8139"));
        
        // Set up network interface
        let time = timer().systime_ms();
        let mut conf = iface::Config::new(HardwareAddress::from(rtl8139.read_mac_address()));
        conf.random_seed = time as u64;

        // The Smoltcp interface struct wants a mutable reference to the device.
        // However, the RTL8139 driver is designed to work with shared references.
        // Since smoltcp does not actually store the mutable reference anywhere,
        // we can safely cast the shared reference to a mutable one.
        // (Actually, I am not sure why the smoltcp interface wants a mutable reference to the device,
        // since it does not modify the device itself.)
        let device = unsafe { ptr::from_ref(rtl8139.deref()).cast_mut().as_mut().unwrap() };
        add_interface(Interface::new(conf, device, Instant::from_millis(time as i64)));

        let sockets = SOCKETS.get().expect("Socket set not initialized!");
        let current_process = process_manager().read().current_process();
        let mut process_map = SOCKET_PROCESS.write();
        // setup DNS
        DNS_SOCKET.call_once(|| {
            let dns_socket = dns::Socket::new(&[], Vec::new());
            let dns_handle = sockets.write().add(dns_socket);
            process_map
                .try_insert(dns_handle, current_process.clone())
                .expect("failed to insert socket into socket-process map");
            dns_handle
        });
        // request an IP address via DHCP
        DHCP_SOCKET.call_once(|| {
            let dhcp_socket = dhcpv4::Socket::new();
            let dhcp_handle = sockets
                .write()
                .add(dhcp_socket);
            process_map
                .try_insert(dhcp_handle, current_process)
                .expect("failed to insert socket into socket-process map");
            dhcp_handle
        });
    }
}

fn check_ownership(handle: SocketHandle) {
    // TODO: these panics should probably kill the process that made the call, not the kernel
    let lock = SOCKET_PROCESS.read();
    let owning_process = lock
        .get(&handle)
        .expect("process tried accessing non-existent socket");
    if *owning_process != process_manager().read().current_process() {
        panic!("process tried to access socket of a different process");
    }
}

// for lifetime-reasons this must be a macro
macro_rules! get_socket_for_current_process {
    ($socket:ident, $handle:ident, $type:ty) => {
        check_ownership($handle);
        let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
        let $socket = sockets.get_mut::<$type>($handle);
    }
}

fn add_interface(interface: Interface) {
    INTERFACES.write().push(interface);
}

/// Get IP addresses for a host.
/// 
/// If host is none, get the addresses of the current host.
pub fn get_ip_addresses(host: Option<&str>) -> Vec<IpAddress> {
    let handle = DNS_SOCKET.get().expect("DNS socket does not exist yet");
    if let Some(host) = host {
        // first, start the queries
        let mut query_handles: Vec<_> = {
            let mut interfaces = INTERFACES.write();
            let interface = interfaces.get_mut(0).expect("network interface is missing");
            let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
            let socket = sockets.get_mut::<dns::Socket>(*handle);
            [DnsQueryType::Aaaa, DnsQueryType::A, DnsQueryType::Cname]
                .into_iter()
                .map(|ty|
                        socket
                            .start_query(interface.context(), host, ty)
                            .map_err(|e| {
                                warn!("DNS query for {host} {ty:?} failed: {e:?}");
                                e
                            })
                            .ok()
                )
                .flatten()
                .collect()
        };
        // then, see if they've returned something
        let mut resulting_ips = Vec::new();
        loop {
            {
                let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
                let socket = sockets.get_mut::<dns::Socket>(*handle);
                let mut remaining: Vec<_> = query_handles
                    .drain(..)
                    .map(|query| match socket.get_query_result(query) {
                        // it's finished, get the results
                        Ok(ips) => {
                            // TODO: does a cname query really return an IP?
                            resulting_ips.extend_from_slice(&ips);
                            None
                        },
                        // if failed, log and and ignore
                        Err(GetQueryResultError::Failed) => {
                            warn!("DNS query for {host} failed");
                            None
                        },
                        // it's still ongoing
                        Err(GetQueryResultError::Pending) => Some(query)
                    })
                    .flatten()
                    .collect();
                if remaining.is_empty() {
                    // we're done!
                    break;
                }
                // else, check for the remaining ones
                query_handles.clear();
                query_handles.append(&mut remaining);
            }
            // release the locks and sleep
            scheduler().sleep(50);
        }
        resulting_ips
    } else {
        INTERFACES
            .read()
            .iter()
            .map(Interface::ip_addrs)
            .flatten()
            .map(IpCidr::address)
            .collect()
    }
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

    let handle = sockets.write().add(udp::Socket::new(rx_buffer, tx_buffer));
    SOCKET_PROCESS
        .write()
        .try_insert(handle, process_manager().read().current_process())
        .expect("failed to insert socket into socket-process map");
    handle
}

pub fn open_tcp() -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    let rx_buffer = tcp::SocketBuffer::new(vec![0; 65535]);
    let tx_buffer = tcp::SocketBuffer::new(vec![0; 65535]);

    let handle = sockets.write().add(tcp::Socket::new(rx_buffer, tx_buffer));
    SOCKET_PROCESS
        .write()
        .try_insert(handle, process_manager().read().current_process())
        .expect("failed to insert socket into socket-process map");
    handle
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

    let handle = sockets.write().add(icmp::Socket::new(rx_buffer, tx_buffer));
    SOCKET_PROCESS
        .write()
        .try_insert(handle, process_manager().read().current_process())
        .expect("failed to insert socket into socket-process map");
    handle
}

pub fn close_socket(handle: SocketHandle) {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    check_ownership(handle);
    SOCKET_PROCESS.write().remove(&handle).unwrap();
    sockets.write().remove(handle);
}

pub fn bind_udp(handle: SocketHandle, addr: IpAddress, port: u16) -> Result<(), udp::BindError> {
    get_socket_for_current_process!(socket, handle, udp::Socket);
    let port = pick_port(port);
    match addr {
        // binding to 0.0.0.0 or :: means listening to all requests
        // but smoltcp doesn't understand it that way
        IpAddress::Ipv4(Ipv4Addr::UNSPECIFIED) | IpAddress::Ipv6(Ipv6Addr::UNSPECIFIED) => socket.bind(port),
        // else, bind to the specified address
        _ => socket.bind((addr, port)),
    }
}

pub fn bind_tcp(handle: SocketHandle, addr: IpAddress, port: u16) -> Result<(), tcp::ListenError> {
    get_socket_for_current_process!(socket, handle, tcp::Socket);
    let port = pick_port(port);
    match addr {
        // binding to 0.0.0.0 or :: means listening to all requests
        // but smoltcp doesn't understand it that way
        IpAddress::Ipv4(Ipv4Addr::UNSPECIFIED) | IpAddress::Ipv6(Ipv6Addr::UNSPECIFIED) => socket.listen(port),
        // else, bind to the specified address
        _ => socket.listen((addr, port)),
    }
}

pub fn bind_icmp(handle: SocketHandle, ident: u16) -> Result<(), icmp::BindError> {
    get_socket_for_current_process!(socket, handle, icmp::Socket);
    socket.bind(icmp::Endpoint::Ident(ident))
}

pub fn accept_tcp(handle: SocketHandle) -> Result<IpEndpoint, tcp::ConnectError> {
    // TODO: smoltcp knows no backlog
    // all but the first connection will fail
    loop {
        // this extra block is needed so that we don't block all sockets
        {
            get_socket_for_current_process!(socket, handle, tcp::Socket);
            if socket.is_active() {
                break;
            }
        }
        scheduler().sleep(100);
    }
    get_socket_for_current_process!(socket, handle, tcp::Socket);
    Ok(socket.remote_endpoint().unwrap())
}

pub fn connect_tcp(handle: SocketHandle, host: IpAddress, port: u16) -> Result<IpEndpoint, tcp::ConnectError> {    get_socket_for_current_process!(socket, handle, tcp::Socket);
    let mut interfaces = INTERFACES.write();
    let interface = interfaces.get_mut(0).ok_or(tcp::ConnectError::InvalidState)?;
    let local_port = pick_port(0);

    socket.connect(interface.context(), (host, port), local_port)?;
    Ok(socket.local_endpoint().unwrap())
}

pub fn send_datagram(handle: SocketHandle, destination: IpAddress, port: u16, data: &[u8]) -> Result<(), udp::SendError> {
    get_socket_for_current_process!(socket, handle, udp::Socket);
    socket.send_slice(data, (destination, port))
}

pub fn send_tcp(handle: SocketHandle, data: &[u8]) -> Result<usize, tcp::SendError> {
    get_socket_for_current_process!(socket, handle, tcp::Socket);
    socket.send_slice(data)
}

pub fn send_icmp(handle: SocketHandle, destination: IpAddress, data: &[u8]) -> Result<(), icmp::SendError> {
    get_socket_for_current_process!(socket, handle, icmp::Socket);
    socket.send_slice(data, destination)
}

pub fn receive_datagram(handle: SocketHandle, data: &mut [u8]) -> Result<(usize, udp::UdpMetadata), udp::RecvError> {
    get_socket_for_current_process!(socket, handle, udp::Socket);
    socket.recv_slice(data)
}

pub fn receive_tcp(handle: SocketHandle, data: &mut [u8]) -> Result<usize, tcp::RecvError> {
    get_socket_for_current_process!(socket, handle, tcp::Socket);
    socket.recv_slice(data)
}

pub fn receive_icmp(handle: SocketHandle, data: &mut [u8]) -> Result<(usize, IpAddress), icmp::RecvError> {
    get_socket_for_current_process!(socket, handle, icmp::Socket);
    socket.recv_slice(data)
}

/// Try to poll all sockets.
/// 
/// This returns None, if it failed to get all needed locks.
/// This is needed, because we otherwise might get a deadlock, because an
/// application has the lock on `sockets` while we have the lock on `interfaces`.
fn poll_sockets() -> Option<()> {
    let rtl8139 = RTL8139.get().expect("RTL8139 not initialized");
    let mut interfaces = INTERFACES.try_write()?;
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").try_write()?;
    let time = Instant::from_millis(timer().systime_ms() as i64);

    // Smoltcp expects a mutable reference to the device, but the RTL8139 driver is built
    // to work with a shared reference. We can safely cast the shared reference to a mutable.
    let device = unsafe { ptr::from_ref(rtl8139.deref()).cast_mut().as_mut().unwrap() };

    let interface = interfaces.get_mut(0).expect("failed to get interface");
    interface.poll(time, device, &mut sockets);
    // DHCP handling is based on https://github.com/smoltcp-rs/smoltcp/blob/main/examples/dhcp_client.rs
    let dhcp_handle = DHCP_SOCKET.get().expect("DHCP socket does not exist yet");
    let dhcp_socket = sockets.get_mut::<dhcpv4::Socket>(*dhcp_handle);
    if let Some(event) = dhcp_socket.poll() {
        match event {
            dhcpv4::Event::Deconfigured => {
                info!("lost DHCP lease");
                interface.update_ip_addrs(|addrs| addrs.clear());
                interface.routes_mut().remove_default_ipv4_route();
            },
            dhcpv4::Event::Configured(config) => {
                info!("acquired DHCP lease:");
                info!("IP address: {}", config.address);
                interface.update_ip_addrs(|addrs| {
                    addrs.clear();
                    addrs.push(IpCidr::Ipv4(config.address)).unwrap();
                });

                if let Some(router) = config.router {
                    info!("default gateway: {router}");
                    interface
                        .routes_mut()
                        .add_default_ipv4_route(router)
                        .unwrap();
                } else {
                    info!("no default gateway");
                    interface
                        .routes_mut()
                        .remove_default_ipv4_route();
                }
                info!("DNS servers: {:?}", config.dns_servers);
                let dns_servers: Vec<_> = config.dns_servers
                    .iter()
                    .map(|ip| IpAddress::Ipv4(*ip))
                    .collect();
                let dns_handle = DNS_SOCKET.get().expect("DNS socket does not exist yet");
                let dns_socket = sockets.get_mut::<dns::Socket>(*dns_handle);
                dns_socket.update_servers(&dns_servers);
            },
        }
    }
    Some(())
}

pub(crate) fn close_sockets_for_process(process: &mut Process) {
    let mut lock = SOCKET_PROCESS.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
    let handles: Vec<_> = lock
        .iter()
        .filter(|(_handle, proc)| ***proc == *process)
        .map(|(handle, _proc)| handle)
        .copied()
        .collect();
    for handle in handles {
        lock.remove(&handle).unwrap();
        sockets.remove(handle);
    }
}

/// Pick a random port if port == 0, else just use the passed port.
fn pick_port(port: u16) -> u16 {
    if port == 0 {
        // TODO: make sure that this isn't used yet
        timer().systime_ms() as u16
    } else {
        port
    }
}
