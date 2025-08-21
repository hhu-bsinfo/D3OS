// =============================================================================
// FILE        : benchmark.rs
// AUTHOR      : Johann Spenrath <johann.spenrath@hhu.de>
// DESCRIPTION : functions for sending and receiving packets and printing stats
// =============================================================================
// NOTES:
// =============================================================================
// DEPENDENCIES:
// =============================================================================
use crate::scheduler;
use crate::{network, timer};
use alloc::vec;
use log::{LevelFilter, debug, info, warn};
use smoltcp::socket::udp::SendError;
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
//use chrono::{DateTime, Utc, Duration};
///////////////////////////////////////////////////////////////
// receiver: bind and print everything arriving on 12345
///////////////////////////////////////////////////////////////
pub fn udp_recv_test() {
    let port = 12344;
    let sock = network::open_socket(network::SocketType::Udp);
    network::bind_udp(sock, port).expect("bind failed");
    // create buffer for printing contents
    let mut buf = [0u8; 1500];

    loop {
        match network::recv_datagram(sock, &mut buf) {
            Ok(Some((len, src_ip, src_port))) => {
                let msg = core::str::from_utf8(&buf[..len]).unwrap_or("<non-utf8>");
                info!("[RX] {}:{} -> {}", src_ip, src_port, msg.trim_end());
            }
            // nothing this tick, background poller will deliver when ready
            Ok(None) => {}
            Err(e) => {
                info!("(UDP Receive Test) receive error: {:?}", e);
            }
        }
        // keep it cooperative; poll thread is already running
        scheduler().sleep(1);
    }
}

///////////////////////////////////////////////////////////////
// sender: fire N packets to 10.0.2.2:12345 and handle backpressure
///////////////////////////////////////////////////////////////
// old test worked until the TX ring filled, then it paniced the kernel because call .expect("Failed to send UDP datagram").
// new version doesn’t crash because it handles backpressure (BufferFull) by polling/yielding and retrying instead of panicking.
pub fn udp_send_test(n: usize) {
    let dst_port = 12345;
    let sock = network::open_socket(network::SocketType::Udp);
    network::bind_udp(sock, dst_port).expect("socket bind failed");

    let dst_ip = Ipv4Address::new(10, 0, 2, 2);
    let datagram: &[u8] = b"Hello from D3OS!\n";

    for _ in 0..n {
        // retry until queued; the poll thread will drain TX between retries
        loop {
            // catch error buffer full by giving the poll method more time
            match network::send_datagram(sock, dst_ip, dst_port, datagram) {
                Ok(()) => break,
                Err(SendError::BufferFull) => {
                    info!("Buffer full");
                    // give the poll method time to flush and to finish ARP, then retry
                    scheduler().sleep(1);
                }
                Err(e) => panic!("(UDP Send Test) send failed: {e:?}"),
            }
            //network::send_datagram(sock, dst_ip, dst_port, datagram);
        }
        // light pacing so the CPU doesn't get hoged
        //scheduler().sleep(10);
    }
}

pub fn client_send() {
    // prepare the init message
    let init_msg = b"Init";
    info!("Init test connection");

    let dst_ip = Ipv4Address::new(10, 0, 2, 2);
    let dst_port = 12345;

    let sock = network::open_socket(network::SocketType::Udp);
    network::bind_udp(sock, dst_port).expect("socket bind failed");
    // send init message to server
    network::send_datagram(sock, dst_ip, dst_port, init_msg);

    // wait for reply from server
    info!("Waiting for Server reply");
}

/*pub fn send_traffic(timing_interval: u16, packet_length: u16) {
    // create the packet
    // in rust indices like vec indexing or slicing have to be of type usize,
    // because usize matches the platform's pointer width and ensures safe, efficient indexing
    let packet_length: usize = packet_length.into();
    let mut packet = vec![0u8; packet_length];

    let mut packet_number: u32 = 0;
    let mut interval_counter = 0;
    let mut bytes_send_interval = 0;
    let port = 12345;
    let sock = network::open_socket(network::SocketType::Udp);
    network::bind_udp(sock, port).expect("socket bind failed");

    let dst_ip = Ipv4Address::new(10, 0, 2, 2);
    let dst_port = 12345;
    let datagram: &[u8] = b"Hello from D3OS!\n";
    let _ = network::send_datagram(sock, dst_ip, dst_port, b"warmup");
    loop {
        match network::send_datagram(sock, dst_ip, dst_port, &packet) {
            Ok(()) => break,
            Err(SendError::BufferFull) | Err(SendError::Unaddressable) => {
                network::poll_sockets(); // pump ARP stack
                scheduler().sleep(5); // small delay
            }
            Err(e) => panic!("send failed: {:?}", e),
        }
    }

    //for i in &mut packet[4..] {
    //    *i = 0;
    //}

    // set interval end
    let mut test_finish_time = timer().systime_ms() + timing_interval as usize; // end of test
    let mut seconds_passed = timer().systime_ms() + 1_000; // next 1s tick

    while test_finish_time > timer().systime_ms() {
        packet_number = packet_number.wrapping_add(1);
        packet[..4].copy_from_slice(&packet_number.to_be_bytes()); // simpler & safer than manual shifts

        network::send_datagram(sock, dst_ip, dst_port, &packet);

        // track bytes sent within interval
        bytes_send_interval += packet_length;

        // if a second has passed write the current bytes per second into the output
        let now = timer().systime_ms();
        if seconds_passed <= now {
            info!(
                "{} - {} : {} KB/s ",
                interval_counter,
                interval_counter + 1,
                bytes_send_interval / 1000
            );
            interval_counter += 1;
            // reset bytes send
            bytes_send_interval = 0;
            // set seconds to next seconds passed
            seconds_passed += 1_000;
        }
    }

    let send_bytes: u32 = packet_length as u32 * packet_number;
    info!("------------------------------------------------------");
    info!("Packets transmitted  : {}", packet_number);
    info!("Bytes transmitted : {} KB", send_bytes / 1000);
    info!(
        "Average           : {} KB/s",
        (send_bytes / timing_interval as u32) / 1000
    );
    info!("------------------------------------------------------");
}
*/
