#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
#[allow(unused_imports)]
use runtime::*;

use ibverbs::*;
use network::{resolve_hostname, TcpListener, TcpStream};
use terminal::println;
use time::get_time_in_us;

const ENCODE_SIZE: usize = 6;
const PORT: u16 = 18515;

fn to_bytes(endpoint: QueuePairEndpoint) -> [u8;ENCODE_SIZE] {
    let mut bytes = [0u8;ENCODE_SIZE];
    bytes[0..4].copy_from_slice(&endpoint.num.to_be_bytes());
    bytes[4..6].copy_from_slice(&endpoint.lid.to_be_bytes());
    bytes
}

fn from_bytes(bytes: [u8;ENCODE_SIZE]) -> QueuePairEndpoint {
    QueuePairEndpoint {
        gid: None,
        num: u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
        lid: u16::from_be_bytes(bytes[4..6].try_into().unwrap()),
    }
}

#[unsafe(no_mangle)]
fn main() {
    let mut args = env::args();
    let _ = args.next().expect("no progname");
    let servername = args.next().filter(|s| s != "--");
    let tx_depth   = args.next().map(|s| s.parse::<u32>().unwrap()).unwrap_or(32);
    let rx_depth   = 4 * tx_depth;
    let iterations = args.next().map(|s| s.parse::<u32>().unwrap()).unwrap_or(1000);
    let bytes      = args.next().map(|s| s.parse::<u32>().unwrap()).unwrap_or(4096);

    let ctx = ibverbs::devices()
        .unwrap()
        .iter()
        .next()
        .expect("no rdma device available")
        .open()
        .unwrap();

    let cq = ctx.create_cq(2*(tx_depth + rx_depth) as i32, 0).unwrap();
    let pd = ctx.alloc_pd().unwrap();

    let qp_builder = pd
        .create_qp(&cq, &cq, QueuePairType::RC)
        .set_max_send_wr(tx_depth)
        .set_max_recv_wr(rx_depth)
        .build()
        .unwrap();

    let mut res = [0u8;ENCODE_SIZE];
    if let Some(ref servername) = servername {
        let ip = resolve_hostname(servername).pop().unwrap();
        let addr = SocketAddr::new(ip, PORT);
        println!("Connecting to {:?}", addr);
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write(&to_bytes(qp_builder.endpoint())).unwrap();
        stream.read(&mut res).unwrap();
    } else {
        println!("Waiting for connections...");
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0,0,0,0)), PORT);
        let mut listener = TcpListener::bind(addr).unwrap();
        let mut stream = listener.accept().unwrap();
        stream.read(&mut res).unwrap();
        stream.write(&to_bytes(qp_builder.endpoint())).unwrap();
    }
    let endpoint = from_bytes(res);
    println!("{:?}", endpoint);
    let mut qp = qp_builder.handshake(endpoint).expect("handshake");

    println!("alloc mr");
    let mut mr = pd.allocate::<u8>(bytes as usize).expect("mr alloc");

    let signal_ration = 2;
    let mut n = 0u32;
    let now = if servername.is_some() {
        mr.fill(42);
        let now = get_time_in_us();
        for _ in 0..tx_depth {
            let flags =  SendFlags::SIGNALED;
            unsafe { qp.post_send([SendWorkRequest::send(1, &[mr.slice(..)], flags)]) }.expect("send");
        }
        now
    } else {
        let now = get_time_in_us();
        for _ in 0..rx_depth {
            unsafe { qp.post_receive([ReceiveWorkRequest { wr_id: 2, sges: &[mr.slice(..)] }]) }.expect("recv");
        }
        now
    };

    let mut completions = [WorkCompletion::default(); 16];
    // in the worst case every iteration generates a single completion
    let mut completion_log = Vec::with_capacity(iterations as usize);
    while n < iterations.div_ceil(signal_ration) {
        let completed = cq.poll(&mut completions[..]).expect("poll");
        if completed.is_empty() {
            continue;
        }
        for _ in 0..completed.len() {
            if servername.is_some() {
                let flags =  SendFlags::SIGNALED;
                unsafe { qp.post_send([SendWorkRequest::send(1, &[mr.slice(..)], flags)]) }.expect("send");
            } else {
                unsafe { qp.post_receive([ReceiveWorkRequest { wr_id: 2, sges: &[mr.slice(..)] }]) }.expect("recv");
            }
        }
        n += completed.len() as u32;
        completion_log.push(completed.len())
    }
    let elapsed = (get_time_in_us() - now) as u128 * 1000;
    let mps = iterations as u128 * 1000_000_000 / elapsed;
    let gbits = bytes as f64 * 8.0 * mps as f64 / (1 << 30) as f64;
    println!("avg completions {}", iterations as f32 / completion_log.len() as f32);
    println!("elapsed: {}ns, bandwidth: {:.3}Gb/s, msgrate: {}msg/s", elapsed, gbits, mps);
}