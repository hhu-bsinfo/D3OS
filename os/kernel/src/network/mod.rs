use crate::device::rtl8139::Rtl8139;
// add the N2000 driver
use crate::device::ne2k::ne2000::Ne2000;
use crate::process::thread::Thread;
use crate::{pci_bus, scheduler, timer};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;
use core::ptr;
use core::sync::atomic::Ordering;
use log::info;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp::{self, UdpMetadata};
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
use spin::{Once, RwLock};

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

    let enable_rtl8139 = true;
    let enable_ne2k = false;

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
                    //poll_sockets();
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

pub fn open_socket(protocol: SocketType) -> SocketHandle {
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
    let rx_buffer =
        udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; rx_size], vec![0; 100000]);

    let tx_size = 1999;
    let tx_buffer = udp::PacketBuffer::new(
        vec![udp::PacketMetadata::EMPTY; tx_size],
        vec![0; 60 * tx_size],
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

    // packets don't hit the wire when calling send_slice, poll() transmits them
    // if poll is to slow, socket tx and rx buffer limit will be reached
    socket.send_slice(data, (destination, port))
}

// disabled for the rtl8139.rs
/*pub fn poll_sockets() {
    //let rtl8139 = RTL8139.get().expect("RTL8139 not initialized");
    if let Some(rtl8139) = RTL8139.get() {
        // Use `rtl`
        let mut interfaces = INTERFACES.write();
        let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
        let time = Instant::from_millis(timer().systime_ms() as i64);
        // Smoltcp expects a mutable reference to the device, but the RTL8139 driver is built
        // to work with a shared reference. We can safely cast the shared reference to a mutable.
        let device = unsafe { ptr::from_ref(rtl8139.deref()).cast_mut().as_mut().unwrap() };
        for interface in interfaces.iter_mut() {
            interface.poll(time, device, &mut sockets);
        }
    } else if let Some(ne2k) = NE2000.get() {
        let mut interfaces = INTERFACES.write();
        let mut sockets = SOCKETS.get().expect("Socket set not initialized!").write();
        let time = Instant::from_millis(timer().systime_ms() as i64);
        //use ne2k
        let device = unsafe { ptr::from_ref(ne2k.deref()).cast_mut().as_mut().unwrap() };
        for interface in interfaces.iter_mut() {
            interface.poll(time, device, &mut sockets);
        }
    }
    {
        // Handle `None`
    }
}*/

// poll for ne2k

fn poll_ne2000_tx() {
    // interface is connection between smoltcp crate and driver
    // interfaces stores a Vector off all added Network Interfaces
    // Cast Arc<Ne2000> to &mut Ne2000 for poll:
    let now = Instant::from_millis(timer().systime_ms() as i64);
    let ne = NE2000
        .get()
        .expect("[poll_ne2000] : Ne2000 not initialized.");
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
    let ne = NE2000
        .get()
        .expect("[poll_ne2000] : Ne2000 not initialized.");
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
    let ne = NE2000
        .get()
        .expect("[poll_ne2000] : Ne2000 not initialized.");
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
