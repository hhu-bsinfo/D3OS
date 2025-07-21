#![no_std]
extern crate alloc;

use core::net::{IpAddr, Ipv4Addr, SocketAddr};

use network::UdpSocket;
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println, read::read};

// TODO: also support TCP

#[unsafe(no_mangle)]
fn main() {
    let mut args = env::args();
    // the first argument is the program name, ignore it
    args.next();
    // the next arguments should be host and port
    if let Some(host) = args.next() && let Some(port_str) = args.next() {
        // TODO: also support host names
        let remote_ip: IpAddr = host.parse().expect("failed to parse IP address");
        let remote_port: u16 = port_str.parse().expect("failed to parse port");
        let remote_addr = SocketAddr::new(remote_ip, remote_port);
        // TODO: this can be any port
        let local_port = remote_port;
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), local_port);
        let socket = UdpSocket::bind(local_addr).expect("failed to open socket");
        let mut buf = [0u8; 1024];
        loop {
            let key = read();
            if let Some(key) = key {
                let string = key.encode_utf8(&mut buf);
                socket.send_to(string.as_bytes(), remote_addr)
                    .expect("failed to send char");
            }
            let (len, _) = socket.recv_from(&mut buf)
                .expect("failed to receive char");
            if len > 0 {
                let text = str::from_utf8(&buf[0..len]).expect("failed to parse received string");
                print!("{text}");
            }
        }
    } else {
        println!("Usage: nc <host> <port>");
    }
}
