use log::debug;
use terminal::println;
use crate::bench::{self, Role};
use crate::cli::ClientArgs;
use crate::comm::{self, BenchmarkRequest, ClientEndpoint, HandshakeAck};
use crate::device;
use crate::error::{other, Result};
use crate::transport;

pub fn run(args: ClientArgs) -> Result<()> {
    let ctx = device::open()?;
    let pd = ctx.alloc_pd()?;
    let cq = ctx.create_cq((2 * args.tx_depth) as i32, 0)?;

    let prepared = transport::build(args.transport, &pd, &cq, args.tx_depth)?;
    let local_endpoint = prepared.endpoint();

    let conn = comm::connect(args.host, args.port)?;
    conn.send_msg(&BenchmarkRequest {
        transport: args.transport,
        mode: args.mode,
        msg_size: args.size,
        iterations: args.iterations,
        tx_depth: args.tx_depth,
    })?;

    let ack: HandshakeAck = conn.recv_msg()?;
    let remote_endpoint = match ack {
        HandshakeAck::Unsupported(reason) => {
            terminal::println!("server rejected benchmark request: {}", reason);
            return Err(other("server rejected benchmark request"));
        }
        HandshakeAck::Ok { endpoint } => endpoint,
    };

    println!("remote-endpoint = {:?}", remote_endpoint);

    let mut qp = prepared.handshake(remote_endpoint)?;
    conn.send_msg(&ClientEndpoint { endpoint: local_endpoint })?;

    bench::run(args.mode, &pd, &cq, &mut qp, &conn, Role::Client, args.size, args.iterations, args.tx_depth)
}
