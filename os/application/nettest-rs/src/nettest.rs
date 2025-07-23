// =============================================================================
// FILE        : nettest.rs
// AUTHOR      : Johann Spenrath
// DESCRIPTION : Rust Implementation of the nettest
//               application, based on the bachelor thesis
//               by Marcel Thiel
// =============================================================================
// TODO: add reference
//
// NOTES:
//
// =============================================================================
// DEPENDENCIES:
// =============================================================================
#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
#[allow(unused_imports)]
use runtime::*;
use smoltcp::iface::{Interface, SocketHandle, SocketSet};
use smoltcp::socket::udp;
use smoltcp::time::Instant;
use smoltcp::wire::Ipv4Address;
use terminal::{print, println};
// get local address
use core::net::SocketAddr;

// Define the Port of the Client
const DEFAULT_BIND_PORT: u32 = 1797;
// Define the Port of the Server
const DEFAULT_REMOTE_PORT: u32 = 1856;
const DEFAULT_PACKET_SIZE: u32 = 1024;
// Define the time interval, in which packets are sent
const DEFAULT_INTERVAL: u32 = 10;

// =============================================================================
// Server Mode
// =============================================================================
pub fn server(socket: &Ud) -> u32 {
    let local_net_address = match socket

    if socket.getLocalAddress {
        println("nettest-rs: Failed to query socket address!");
        return -1;
    }

    println("nettest: sever listening on {} ", local_net_address);
    println("Send 'exit' to leave.");
}

/*
    // Wait for client to initiate connection, return if exit code is != 0
    while (true) {
        auto receivedDatagram = Util::Network::Udp::UdpDatagram();
        if (!socket.receive(receivedDatagram)) {
            Util::System::error << "nettest: Failed to receive echo request!" << Util::Io::PrintStream::endl << Util::Io::PrintStream::flush;
            return -1;
        }

        // If connection request is received: send reply to client
        if (Util::String(receivedDatagram.getData(), receivedDatagram.getLength()).strip() == "Init") {
            if (!socket.send(receivedDatagram)) {
                Util::System::error << "nettest: Failed to send echo reply!" << Util::Io::PrintStream::endl << Util::Io::PrintStream::flush;
                return -1;
            }

            return receiveTraffic(socket);
        } else if (Util::String(receivedDatagram.getData(), receivedDatagram.getLength()).strip() == "InitR") { /** Reverse test: */
            if (!socket.send(receivedDatagram)) {
                Util::System::error << "nettest: Failed to send echo reply!" << Util::Io::PrintStream::endl << Util::Io::PrintStream::flush;
                return -1;
            }

            // Wait for message with packetLength and timing interval
            if (!socket.receive(receivedDatagram)) {
                Util::System::error << "nettest: Failed to receive echo request!" << Util::Io::PrintStream::endl
                                    << Util::Io::PrintStream::flush;
                return -1;
            }
            if(receivedDatagram.getLength() != 4){
                Util::System::error << "nettest: Failed to receive reverse test data! " << Util::Io::PrintStream::endl << Util::Io::PrintStream::flush;
                return -1;
            }

            // Get packetLen and Test duration
            auto data = receivedDatagram.getData();
            uint16_t packetLength = (data[0] << 8) + data[1];
            uint16_t timingInterval = (data[2] << 8) + data[3];

            // Get destination address from client
            auto destinationAddress = reinterpret_cast<const Util::Network::Ip4::Ip4PortAddress&>(receivedDatagram.getRemoteAddress());
            destinationAddress.setPort(receivedDatagram.getRemotePort());
            // Start reverse test
            return send_traffic(socket, destinationAddress, timingInterval, packetLength);
        }
    }
}*/

#[unsafe(no_mangle)]
pub fn main() {
    println!("nettest-rs");
}
