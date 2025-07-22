#![no_std]
extern crate alloc;

use core::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use alloc::string::String;
use network::{TcpListener, TcpStream, UdpSocket};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println, read::read};

enum Protocol {
    Udp, Tcp,
}

enum Mode {
    Listen,
    Connect,
}

enum Socket {
    Udp(UdpSocket),
    Tcp(TcpStream),
}

#[unsafe(no_mangle)]
fn main() {
    let mut args = env::args().peekable();
    // the first argument is the program name, ignore it
    args.next();

    let mut mode = Mode::Connect;
    let mut protocol = Protocol::Tcp;

    // check the next arguments for flags
    loop {
        match args.peek().map(String::as_str) {
            Some("-h") | Some("--help") => {
                println!("Usage:
    nc [-u] [-l] host port

Examples:
    nc 1.2.3.4 5678
        open a TCP connection to 1.2.3.4:5678
    nc -u -l 0.0.0.0 1234
        bind to 0.0.0.0:1234, UDP");
                return;
            }
            Some("-l") => {
                mode = Mode::Listen;
                args.next();
            },
            Some("-u") => {
                protocol = Protocol::Udp;
                args.next();
            },
            // now, we're finally past the options
            Some(_) => break,
            None => {
                println!("Usage: nc [-u] [-l] host port");
                return;
            },
        }
    }

    // the next arguments should be host and port
    // for listen, this is the address and port to bind to
    // for connect, this is the remote host to connect to
    let addr = if let Some(host) = args.next() && let Some(port_str) = args.next() {
        // TODO: also support host names
        let ip: IpAddr = host.parse().expect("failed to parse IP address");
        let port: u16 = port_str.parse().expect("failed to parse port");
        SocketAddr::new(ip, port)
    } else {
        println!("Usage: nc [-u] [-l] host port");
        return;
    };

    let socket = match mode {
        Mode::Listen => match protocol {
            Protocol::Udp => Socket::Udp(UdpSocket::bind(addr).expect("failed to open socket")),
            Protocol::Tcp => Socket::Tcp(
                TcpListener::bind(addr)
                    .expect("failed to open socket")
                    .accept()
                    .expect("failed to accept connection")
                ),
        },
        Mode::Connect => match protocol {
            Protocol::Udp => {
                // TODO: randomize this, but probably in the kernel?
                let local_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 1797));
                Socket::Udp(UdpSocket::bind(local_addr).expect("failed to open socket"))
            },
            Protocol::Tcp => Socket::Tcp(TcpStream::connect(addr).expect("failed to open socket")),
        },
    };

    // loop: send and receive
    let mut buf = [0u8; 1024];
    loop {
        let key = read();
        if let Some(key) = key {
            let string = key.encode_utf8(&mut buf);
            match socket {
                Socket::Udp(ref sock) => sock.send_to(string.as_bytes(), addr)
                    .expect("failed to send char"),
                Socket::Tcp(ref sock) => sock.write(string.as_bytes())
                    .expect("failed to send char"),
            };
        }
        let len = match socket {
            Socket::Udp(ref sock) => sock.recv_from(&mut buf)
                .expect("failed to receive char").0,
            Socket::Tcp(ref sock) => sock.read(&mut buf)
                .expect("failed to receive char"),
        };
        if len > 0 {
            let text = str::from_utf8(&buf[0..len]).expect("failed to parse received string");
            print!("{text}");
        }
    }

}
