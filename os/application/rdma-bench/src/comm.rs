//! Out-of-band TCP handshake, newline-delimited-JSON-encoded (mirrors `rust-rdma-bench`'s
//! `comm.rs` on the wire, so a D3OS peer and a native `rust-rdma-bench` peer can talk to each
//! other) — see `rdma/mlx4/src/handshake.rs` for the unrelated bincode-based kernel<->userspace
//! handshake, which this does not touch.

use crate::cli::{Mode, Transport};
use crate::error::{other, Result};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use concurrent::thread::sleep;
use core::cell::RefCell;
use core::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use ibverbs::{QueuePairEndpoint, RemoteMemoryRegion};
use network::{TcpListener, TcpStream};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Declares the benchmark a client wants to run. Sent by the client as the first message on a
/// new connection, before either side has built any RDMA resources for that connection.
#[derive(Serialize, Deserialize, Debug)]
pub struct BenchmarkRequest {
    pub transport: Transport,
    pub mode: Mode,
    pub msg_size: usize,
    pub iterations: usize,
    pub tx_depth: usize,
}

/// The server's reply to a `BenchmarkRequest`.
#[derive(Serialize, Deserialize, Debug)]
pub enum HandshakeAck {
    Ok { endpoint: QueuePairEndpoint },
    Unsupported(String),
}

/// The client's queue pair endpoint, sent back to the server after the client has consumed the
/// server's `HandshakeAck::Ok`.
#[derive(Serialize, Deserialize, Debug)]
pub struct ClientEndpoint {
    pub endpoint: QueuePairEndpoint,
}

/// Sent from the rdma-write/rdma-read responder to the initiator once its buffer is registered,
/// authorizing the initiator's HCA to write/read it directly without any further involvement from
/// the responder's CPU.
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteBufferInfo {
    pub remote: RemoteMemoryRegion<u8>,
}

/// Sent from the accuracy-mode receiver back to the sender once its drain loop finishes.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct AccuracyReport {
    pub msg_size: usize,
    pub sent: usize,
    pub received: usize,
    pub lost: usize,
    pub duplicated: usize,
    pub unidentifiable: usize,
    pub truncated: usize,
    pub corrupted: usize,
    pub correct_bytes: u64,
    pub correct_bits: u64,
}

const RETRY_MS: usize = 20;

/// A single out-of-band TCP connection used to exchange handshake messages and barriers before
/// (and after) an RDMA benchmark run.
pub struct Conn {
    stream: TcpStream,
    /// Bytes already read from the socket but not yet consumed by a `recv_msg`/`sync` call — a
    /// single `read()` can return more or less than one newline-delimited message.
    pending: RefCell<VecDeque<u8>>,
}

impl Conn {
    fn new(stream: TcpStream) -> Self {
        Self { stream, pending: RefCell::new(VecDeque::new()) }
    }

    pub fn send_msg<T: Serialize>(&self, msg: &T) -> Result<()> {
        let mut line = serde_json::to_string(msg).map_err(|_| other("json encode failed"))?;
        line.push('\n');
        self.write_all(line.as_bytes())
    }

    pub fn recv_msg<T: DeserializeOwned>(&self) -> Result<T> {
        loop {
            if let Some(line) = self.take_line() {
                return serde_json::from_slice(&line).map_err(|_| other("json decode failed"));
            }
            self.fill_pending()?;
        }
    }

    /// A one-byte round trip both sides call at the same logical point, so neither proceeds past
    /// it until the other has reached it too.
    pub fn sync(&self) -> Result<()> {
        self.write_all(&[0u8])?;
        loop {
            if self.pending.borrow_mut().pop_front().is_some() {
                return Ok(());
            }
            self.fill_pending()?;
        }
    }

    /// Pulls the next complete `\n`-terminated message out of `pending`, if there is one, leaving
    /// the newline itself and everything after it in the queue.
    fn take_line(&self) -> Option<Vec<u8>> {
        let mut pending = self.pending.borrow_mut();
        let pos = pending.iter().position(|&b| b == b'\n')?;
        let line: Vec<u8> = pending.drain(..pos).collect();
        pending.pop_front(); // discard the newline
        Some(line)
    }

    /// Reads whatever is currently available and appends it to `pending`.
    fn fill_pending(&self) -> Result<()> {
        let mut buf = [0u8; 1024];
        let n = self.read_some(&mut buf)?;
        self.pending.borrow_mut().extend(buf[..n].iter().copied());
        Ok(())
    }

    fn write_all(&self, data: &[u8]) -> Result<()> {
        let mut sent = 0;
        while sent < data.len() {
            if let Ok(true) = self.stream.can_send() {
                match self.stream.write(&data[sent..]) {
                    Ok(n) => sent += n,
                    Err(_) => return Err(other("network write failed")),
                }
            } else {
                sleep(RETRY_MS);
            }
        }
        // `write` only hands the bytes to the network stack; there is no flush in the `network`
        // API to wait on them actually leaving. After a barrier the caller goes straight into a
        // tight completion-polling loop and stops touching the socket, so give the stack a chance
        // to transmit before that happens — otherwise the peer waits on a byte that is sitting in
        // a send buffer.
        sleep(RETRY_MS);
        Ok(())
    }

    fn read_some(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            if let Ok(true) = self.stream.can_recv() {
                match self.stream.read(buf) {
                    Ok(n) if n > 0 => return Ok(n),
                    Ok(_) => {}
                    Err(_) => return Err(other("network read failed")),
                }
            }
            sleep(RETRY_MS);
        }
    }
}

pub fn listen(port: u16) -> Result<TcpListener> {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
    TcpListener::bind(addr).map_err(|_| other("failed to bind TCP listener"))
}

pub fn accept_one(listener: &mut TcpListener) -> Result<Conn> {
    let stream = listener.accept().map_err(|_| other("failed to accept TCP connection"))?;
    Ok(Conn::new(stream))
}

pub fn connect(host: IpAddr, port: u16) -> Result<Conn> {
    let stream = TcpStream::connect(SocketAddr::new(host, port)).map_err(|_| other("failed to connect"))?;
    Ok(Conn::new(stream))
}
