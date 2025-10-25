use log::{debug};
use smoltcp::socket::udp;
use bincode::{decode_from_slice, encode_into_slice, config::standard};
use super::session::UdpSession;
use crate::infiniband::ibverbs::{
    RemoteMemoryRegion, QueuePairEndpoint
};

const READY_MSG: [u8; 10] = *b"READYHERE!";
const ACK: [u8; 6] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

pub fn send_ready_and_wait_ack(session: &UdpSession, retry_interval_ms: usize, max_wait_ms: usize) {
    let mut ack_buf = [0u8; 6];
    let mut ack_received = false;
    loop {
        session.send(&READY_MSG[..])
            .expect("failed to send READY message");
            
        debug!("READY message sent to receiver");

        let mut elapsed = 0;
    
        while elapsed < max_wait_ms {
            match session.recv(&mut ack_buf) {
                Ok((n, _)) if n == ACK.len() && ack_buf == ACK => {
                    debug!("Received ACK from receiver, ready to send data");
                    ack_received = true;
                    break;
                }
                Ok((_, _)) => {
                    //debug!("Received unexpected data, ignoring");
                },
                Err(_) => { 
                    crate::timer().wait(retry_interval_ms); 
                    elapsed += retry_interval_ms; 
                }
            }
        }

        if ack_received {
            break;
        }

        debug!("No ACK yet, resending READY message...");
    }
}

pub fn send_ack(session: &UdpSession) {
    let ack_buf = ACK;
    session.send(&ack_buf[..]).expect("failed to send ACK");
    debug!("ACK sent to sender");
}

pub fn wait_ack(session: &UdpSession) {
    let mut ack_buf = [0u8; 6];
    loop {
        match session.recv(&mut ack_buf) {
            Ok((n, _)) if n == ACK.len() && ack_buf == ACK => {
                debug!("Received ACK, handshake complete");
                break;
            }
            Ok((_, _)) => {
                //debug!("Received unexpected data, ignoring");
            },
            Err(_err) => {
                crate::timer().wait(100);
            }
        }
    }
}

pub fn wait_ready(session: &UdpSession) {
    debug!("Waiting for READY message...");
    let mut buffer = [0u8; 1024];
    loop {
        match session.recv(&mut buffer) {
            Ok((n, _)) if n == READY_MSG.len() && buffer[..n] == READY_MSG[..] => {
                debug!("Received READY message from sender");
                break;
            }
            Ok((_, _)) => {
                //debug!("Received unexpected data, ignoring");
            },
            Err(_err) => {
                crate::timer().wait(100);
            }
        }
    }
}

pub fn exchange_endpoints(
    session: &UdpSession,
    local_ep: QueuePairEndpoint,
) -> QueuePairEndpoint {
    let config = standard()
        .with_big_endian()
        .with_fixed_int_encoding()
        .with_limit::<1024>();

    let mut buf = [0u8; 1024];
    let used = encode_into_slice(local_ep, &mut buf, config).unwrap();
    
    debug!("Sending endpoint ({} bytes) to {}:{}", used, session.ip, session.tgt_port);
    match session.send(&buf[..used]) {
        Ok(()) => debug!("Endpoint sent successfully"),
        Err(e) => debug!("Failed to send endpoint: {:?}", e)
    }

    debug!("Waiting for remote endpoint...");
    let size = loop {
        match session.recv(&mut buf[..]) {
            Ok((n, _)) => {
                debug!("Received {} bytes for endpoint", n);
                break n;
            },
            Err(err) => {
                // debug!("No data for endpoint: {:?}", err);
                crate::timer().wait(100);
            }
        }
    };

    let (remote_ep, _): (QueuePairEndpoint, usize) =
        decode_from_slice(&buf[..size], config).unwrap();
    remote_ep
}

pub fn exchange_memory_region(
    session: &UdpSession,
    local_mr: RemoteMemoryRegion<u8>
) -> RemoteMemoryRegion<u8> {
    let config = standard()
        .with_big_endian()
        .with_fixed_int_encoding()
        .with_limit::<1024>();

    let mut buf = [0u8; 1024];
    let used = encode_into_slice(local_mr, &mut buf, config).unwrap();
    debug!("Sending memory region ({} bytes) to {}:{}", used, session.ip, session.tgt_port);
    match session.send(&buf[..used]) {
        Ok(()) => debug!("Memory region sent successfully"),
        Err(e) => debug!("Failed to send memory region: {:?}", e),
    }

    debug!("Waiting for remote memory region...");
    let size = loop {
        match session.recv(&mut buf) {
            Ok((n, _)) => {
                debug!("Received {} bytes for memory region", n);
                break n;
            },
            Err(err) => {
                debug!("No data for memory region: {:?}", err);
                crate::timer().wait(100);
            }
        }
    };

    let (remote_mr, _): (RemoteMemoryRegion<u8>, usize) =
        decode_from_slice(&buf[..size], config).unwrap();
    remote_mr
}