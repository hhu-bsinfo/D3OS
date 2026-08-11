use crate::bench::{self, Role};
use crate::cli::ServerArgs;
use crate::comm::{self, BenchmarkRequest, ClientEndpoint, Conn, HandshakeAck};
use crate::device;
use crate::error::Result;
use crate::transport;
use alloc::format;
use ibverbs::{Context, ProtectionDomain};
use log::error;

pub fn run(args: ServerArgs) -> Result<()> {
    let ctx = device::open()?;
    let pd = ctx.alloc_pd()?;
    let mut listener = comm::listen(args.port)?;

    terminal::println!("listening on port {}", args.port);
    loop {
        let conn = comm::accept_one(&mut listener)?;
        if let Err(e) = handle_connection(&ctx, &pd, &conn) {
            error!("connection error: {}", e);
            terminal::println!("connection error: {:?}", e);
        }

        if !args.listen {
            break;
        }
        terminal::println!("waiting for next connection...");
    }

    Ok(())
}

fn handle_connection(ctx: &Context, pd: &ProtectionDomain, conn: &Conn) -> Result<()> {
    let req: BenchmarkRequest = conn.recv_msg()?;
    terminal::println!("benchmark request: {:?}", req);

    if !bench::supported(req.transport, req.mode) {
        let reason = format!("{:?}/{:?} is not implemented yet", req.transport, req.mode);
        conn.send_msg(&HandshakeAck::Unsupported(reason))?;
        return Ok(());
    }

    let cq = ctx.create_cq((2 * req.tx_depth) as i32, 0)?;
    let prepared = transport::build(req.transport, pd, &cq, req.tx_depth)?;
    let local_endpoint = prepared.endpoint();
    conn.send_msg(&HandshakeAck::Ok { endpoint: local_endpoint })?;

    let ClientEndpoint { endpoint: remote_endpoint } = conn.recv_msg()?;
    let mut qp = prepared.handshake(remote_endpoint)?;

    // The server side is the passive peer in every mode, so its report carries no numbers of its
    // own — whatever it has to say it has already printed.
    bench::run(req.mode, pd, &cq, &mut qp, conn, Role::Server, req.msg_size, req.iterations, req.tx_depth)?;
    Ok(())
}
