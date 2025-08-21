use crate::device::rtl8139::Rtl8139;
use alloc::collections::btree_map::BTreeMap;
// add the N2000 driver
use crate::device::ne2k::ne2000::Ne2000;
use crate::process::process::Process;
use crate::process::thread::Thread;
use crate::{pci_bus, process_manager, scheduler, timer};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::net::{Ipv4Addr, Ipv6Addr};
use core::ops::Deref;
use core::ptr;
use core::sync::atomic::Ordering;
use log::{info, warn};
use smoltcp::iface::{self, Interface, SocketHandle, SocketSet};
use smoltcp::socket::dns::GetQueryResultError;
use smoltcp::socket::{dhcpv4, dns, icmp, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{DnsQueryType, HardwareAddress, IpAddress, IpCidr, IpEndpoint};
use spin::{Once, RwLock};

static RTL8139: Once<Arc<Rtl8139>> = Once::new();
// ensure that the driver is only initialized once
static NE2000: Once<Arc<Ne2000>> = Once::new();

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
    Udp,
    Tcp,
    Icmp,
}

pub fn init() {
    SOCKETS.call_once(|| RwLock::new(SocketSet::new(Vec::new())));

    let enable_rtl8139 = true;
    let enable_ne2k = false;

    if enable_rtl8139 {
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
            scheduler().ready(Thread::new_kernel_thread(
                || loop {
                    //poll_sockets();
                },
                "RTL8139",
            ));
        }
    }

    if let Some(rtl8139) = RTL8139.get() {
        extern "sysv64" fn poll() {
            loop {
                poll_sockets();
                scheduler().sleep(50);
            }
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
            let dhcp_handle = sockets.write().add(dhcp_socket);
            process_map
                .try_insert(dhcp_handle, current_process)
                .expect("failed to insert socket into socket-process map");
            dhcp_handle
        });
    }
    // Register the Ne2000 card here
    // wrap into Arc for shared ownership
    // Scans PCI bus for Ne2000 cards or similar by looking at the device id and vendor id.
    // References:
    //             - https://en.wikibooks.org/wiki/QEMU/Devices/Network -> the nic model
    //             - https://theretroweb.com/chips/4692 -> device id and vendor id
    if enable_ne2k {
        // get the EndpointHeader
        // the endpoint header contains essential information about the device,
        // such as the Vendor ID (VID), Device ID (DID), and other configuration parameters
        let device_ne2k = pci_bus().search_by_ids(0x10ec, 0x8029);
        if device_ne2k.len() > 0 {
            // perform the initialization only once!
            NE2000.call_once(|| {
                info!("\x1b[1;31mFound Realtek 8029 network controller");
                // initialize the driver
                let device = Ne2000::new(device_ne2k[0]);
                // wrap the instance in an Arc for sharing in a multithreaded context
                let ne2k = Arc::new(device);

                //read the mac address
                info!("\x1b[1;31mNe2000 MAC address: [{}]", ne2k.get_mac());
                //enable interrupt handler
                Ne2000::assign(Arc::clone(&ne2k));
                info!("assigned Interrupt handler");

                // create new thread which polls for the fields rcv and ovw
                // if the trigger method gets called by an packet received
                // or receive buffer overwrite interrupt, the check method
                // calls the corresponding method to handle the interrupt
                scheduler().ready(Thread::new_kernel_thread(
                    || loop {
                        check();
                    },
                    "check",
                ));
                // return the instance
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
                    poll_sockets();
                    // add this because if not, the send test is slowed down
                    // avoid CPU starvation and keep polling regular
                    // prevents CPU hogging by the poll thread.
                    // allows other tasks (like your sender or interrupt handlers) to run.
                    // forms a cooperative multitasking environment for fair scheduling.
                    //scheduler().sleep(1);
                },
                "NE2000_rx",
            ));
            /*scheduler().ready(Thread::new_kernel_thread(
                || loop {
                    poll_ne2000_tx();
                },
                "NE2000_tx",
            ));*/
        }
    }
}

fn check_ownership(handle: SocketHandle) {
    // TODO: these panics should probably kill the process that made the call, not the kernel
    let lock = SOCKET_PROCESS.read();
    let owning_process = lock.get(&handle).expect("process tried accessing non-existent socket");
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
    };
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

pub fn check() {
    let ne2000 = NE2000.get().expect("NE2000 not initialized");
    let device = unsafe { ptr::from_ref(ne2000.deref()).cast_mut().as_mut().unwrap() };
    // check if interrupt occured
    // Packet received ?
    if device.check_interrupts.prx.load(Ordering::Relaxed) {
        device.check_interrupts.prx.store(false, Ordering::Relaxed);
        device.receive_packet();
        // reset the AtomicBool after handling the interrupt
    }
    // Receive Buffer Overwrite?
    if device.check_interrupts.ovw.load(Ordering::Relaxed) {
        // reset the AtomicBool after handling the interrupt
        device.check_interrupts.ovw.store(false, Ordering::Relaxed);
        device.handle_overflow();
    }
}
pub fn add_interface(interface: Interface) {
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
                .filter_map(|ty| {
                    socket
                        .start_query(interface.context(), host, ty)
                        .map_err(|e| {
                            warn!("DNS query for {host} {ty:?} failed: {e:?}");
                            e
                        })
                        .ok()
                })
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
                    .filter(|query| match socket.get_query_result(*query) {
                        // it's finished, get the results
                        Ok(ips) => {
                            // TODO: does a cname query really return an IP?
                            resulting_ips.extend_from_slice(&ips);
                            false
                        }
                        // if failed, log and and ignore
                        Err(GetQueryResultError::Failed) => {
                            warn!("DNS query for {host} failed");
                            false
                        }
                        // it's still ongoing
                        Err(GetQueryResultError::Pending) => true,
                    })
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
        INTERFACES.read().iter().flat_map(Interface::ip_addrs).map(IpCidr::address).collect()
    }
}

pub fn open_udp() -> SocketHandle {
    let sockets = SOCKETS.get().expect("Socket set not initialized!");
    // changed transmit and receive buffer size to tx_size and rx_size
    ////// IMPORTANT//////
    ///// Metadata storage limits the maximum number of packets in the buffer
    /// Limits how many packets can be queued, regardless of size
    ///// and payload storage limits the maximum total size of packets.
    /// Limits total bytes across all packets, ensuring memory bounds are respected
    // https://docs.rs/smoltcp/latest/smoltcp/storage/struct.PacketBuffer.html
    // Problem:  enqueue faster than poll() can transmit,
    // hit whichever limit comes first and get BufferFull
    let rx_size = 1000;
    let rx_buffer = udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; rx_size], vec![0; 100000]);

    let tx_size = 1999;
    let tx_buffer = udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; tx_size], vec![0; 60 * tx_size]);

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

    let rx_buffer = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY, icmp::PacketMetadata::EMPTY], vec![0; 65535]);
    let tx_buffer = icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY, icmp::PacketMetadata::EMPTY], vec![0; 65535]);

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

pub fn connect_tcp(handle: SocketHandle, host: IpAddress, port: u16) -> Result<IpEndpoint, tcp::ConnectError> {
    get_socket_for_current_process!(socket, handle, tcp::Socket);
    let mut interfaces = INTERFACES.write();
    let interface = interfaces.get_mut(0).ok_or(tcp::ConnectError::InvalidState)?;
    let local_port = pick_port(0);

    socket.connect(interface.context(), (host, port), local_port)?;
    Ok(socket.local_endpoint().unwrap())
}

pub fn send_datagram(handle: SocketHandle, destination: IpAddress, port: u16, data: &[u8]) -> Result<(), udp::SendError> {
    get_socket_for_current_process!(socket, handle, udp::Socket);
    // packets don't hit the wire when calling send_slice, poll() transmits them
    // if poll is to slow, socket tx and rx buffer limit will be reached
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
            }
            dhcpv4::Event::Configured(config) => {
                info!("acquired DHCP lease:");
                info!("IP address: {}", config.address);
                interface.update_ip_addrs(|addrs| {
                    addrs.clear();
                    addrs.push(IpCidr::Ipv4(config.address)).unwrap();
                });

                if let Some(router) = config.router {
                    info!("default gateway: {router}");
                    interface.routes_mut().add_default_ipv4_route(router).unwrap();
                } else {
                    info!("no default gateway");
                    interface.routes_mut().remove_default_ipv4_route();
                }
                info!("DNS servers: {:?}", config.dns_servers);
                let dns_servers: Vec<_> = config.dns_servers.iter().map(|ip| IpAddress::Ipv4(*ip)).collect();
                let dns_handle = DNS_SOCKET.get().expect("DNS socket does not exist yet");
                let dns_socket = sockets.get_mut::<dns::Socket>(*dns_handle);
                dns_socket.update_servers(&dns_servers);
            }
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

// poll for ne2k

fn poll_ne2000_tx() {
    // interface is connection between smoltcp crate and driver
    // interfaces stores a Vector off all added Network Interfaces
    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000.get().expect("[poll_ne2000] : Ne2000 not initialized.");
    let dev_ne2k = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };

    // initialize Interfaces and sockets
    let mut interfaces = INTERFACES.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized").write();

    // start interface
    // iterate through every interface and call the poll method
    for iface in interfaces.iter_mut() {
        //info!("Polling, Iteration: {}", counter);
        // check if smoltcp processes something
        // poll calls the receive and transmit methods of the device impl in networks_stack/mod.rs
        // checking if any packets have been send or received
        //let timestamp = Instant::now();
        //iface.poll(now, dev_ne2k, &mut sockets);
        iface.poll_egress(now, dev_ne2k, &mut sockets);
        //iface.poll_ingress_single(now, dev_ne2k, &mut sockets);

        // 09.08.2025
        // https://docs.rs/smoltcp/latest/smoltcp/iface/struct.Interface.html#method.poll_delay
        //let delay = iface.poll_delay(timestamp, &sockets).unwrap_or_default();
        //scheduler().sleep(delay);
    }
}

fn poll_ne2000_rx() {
    // interface is connection between smoltcp crate and driver
    // interfaces stores a Vector off all added Network Interfaces
    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000.get().expect("[poll_ne2000] : Ne2000 not initialized.");
    let dev_ne2k = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };

    // initialize Interfaces and sockets
    let mut interfaces = INTERFACES.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized").write();

    // start interface
    // iterate through every interface and call the poll method
    for iface in interfaces.iter_mut() {
        //info!("Polling, Iteration: {}", counter);
        // check if smoltcp processes something
        // poll calls the receive and transmit methods of the device impl in networks_stack/mod.rs
        // checking if any packets have been send or received
        //let timestamp = Instant::now();
        //iface.poll(now, dev_ne2k, &mut sockets);
        //iface.poll_egress(now, dev_ne2k, &mut sockets);
        iface.poll_ingress_single(now, dev_ne2k, &mut sockets);

        // 09.08.2025
        // https://docs.rs/smoltcp/latest/smoltcp/iface/struct.Interface.html#method.poll_delay
        //let delay = iface.poll_delay(timestamp, &sockets).unwrap_or_default();
        //scheduler().sleep(delay);
    }
}

fn poll_sockets() {
    // Tune this cap if bursts are heavy:
    const MAX_INGRESS_PER_TICK: usize = 8;
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000.get().expect("[poll_ne2000] : Ne2000 not initialized.");
    let dev_ne2k = unsafe { ptr::from_ref(ne.deref()).cast_mut().as_mut().unwrap() };

    // initialize Interfaces and sockets
    let mut interfaces = INTERFACES.write();
    let mut sockets = SOCKETS.get().expect("Socket set not initialized").write();

    for iface in interfaces.iter_mut() {
        // 1) Always flush all outbound packets (bounded work by design)
        let _ = iface.poll_egress(now, dev_ne2k, &mut sockets);

        // 2) Then handle at most N ingress packets this tick
        for _ in 0..MAX_INGRESS_PER_TICK {
            match iface.poll_ingress_single(now, dev_ne2k, &mut sockets) {
                //This contains information on whether a packet was processed or not,
                //and whether it might’ve affected socket states.
                smoltcp::iface::PollIngressSingleResult::PacketProcessed { .. } => {
                    // Optionally interleave another egress flush so replies go out promptly
                    let _ = iface.poll_egress(now, dev_ne2k, &mut sockets);
                }
                smoltcp::iface::PollIngressSingleResult::None => break,
                smoltcp::iface::PollIngressSingleResult::SocketStateChanged => {
                    let _ = iface.poll_ingress_single(now, dev_ne2k, &mut sockets);
                }
            }
        }

        // 3) Pace the loop (optional but recommended)
        if let Some(delay) = iface.poll_delay(now, &sockets) {
            // sleep/yield according to your scheduler
            //scheduler().sleep(delay);
        }
    }
}
