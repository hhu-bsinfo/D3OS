//! ping – send and receive ICMP echo requests
//! 
//! This is based on the [smoltcp echo example](https://github.com/smoltcp-rs/smoltcp/blob/main/examples/ping.rs).
#![no_std]
extern crate alloc;

use alloc::{string::String, vec};
use concurrent::thread::sleep;
use network::{resolve_hostname, IcmpSocket};
#[allow(unused_imports)]
use runtime::*;
use smoltcp::{phy::ChecksumCapabilities, wire::{Icmpv4Packet, Icmpv4Repr}};
use terminal::{print, println};

#[unsafe(no_mangle)]
fn main() {
    let mut args = env::args().peekable();
    // the first argument is the program name, ignore it
    args.next();

    let mut count = 5;

    // check the next arguments for flags
    loop {
        match args.peek().map(String::as_str) {
            Some("-h") | Some("--help") => {
                println!("Usage:
    ping [-c count] host

Examples:
    ping -c 2 1.2.3.4
        ping 1.2.3.4 two times");
                return;
            }
            Some("-c") => {
                args.next();
                count = args.next().unwrap().parse().unwrap();
            },
            // now, we're finally past the options
            Some(_) => break,
            None => {
                println!("Usage: ping [-c count] host");
                return;
            },
        }
    }

    // the next argument should be the host
    let Some(host) = args.next() else {
        println!("Usage: ping [-c count] host");
        return;
    };
    // just take the first IP address
    let ip = resolve_hostname(&host).into_iter().next().unwrap();

    let ident = 0x1234;
    let socket = IcmpSocket::bind(ident).expect("failed to open socket");
    for seq_no in 0..count {
        let send_time: [u8; 8] = time::date().timestamp_millis().to_ne_bytes();
        // TODO: IPv6
        let request = Icmpv4Repr::EchoRequest { ident, seq_no, data: &send_time };
        let mut packet_buffer = vec![0u8; request.buffer_len()];
        let mut packet = Icmpv4Packet::new_checked(&mut packet_buffer).unwrap();
        request.emit(&mut packet, &ChecksumCapabilities::ignored());
        socket.send_to(&packet_buffer, ip).expect("failed to send ping");

        let mut recv_buffer = [0u8; 4096];
        let addr = loop {
            let (len, addr) = socket
                .recv(&mut recv_buffer)
                .expect("failed to receive ping reply");
            if len != 0 {
                break addr;
            }
            sleep(50);
        };
        let response_packet = Icmpv4Packet::new_checked(&recv_buffer).expect("received packet is invalid");
        let response = Icmpv4Repr::parse(&response_packet, &ChecksumCapabilities::ignored()).expect("received packet is invalid");
        if let Icmpv4Repr::EchoReply { seq_no, data, .. } = response {
            let timestamp_ms = i64::from_ne_bytes(data[0..8].try_into().unwrap());
            let timedelta = time::date().timestamp_millis() - timestamp_ms;
            println!("{} bytes from {}: seq={}, time={}ms", data.len(), addr, seq_no, timedelta);
        } else {
            println!("ignoring unexpected ICMP packet")
        }
    }
}
