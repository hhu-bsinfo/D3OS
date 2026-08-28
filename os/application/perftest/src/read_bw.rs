use core::{net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4}, str::FromStr};

use alloc::string::String;
use core3::io;
use core3::io::ErrorKind;
use ibverbs::{ibv_qp_type, Gid, LocalMemoryRegion, QueuePairEndpoint};
use network::{TcpListener, TcpStream};
use rdma::ib_core::ibv_qp_cap;
use terminal::println;

use crate::comm::{self, PeerInfo};

const DEFAULT_PORT: u16 = 18515;
const DEFAULT_GID_INDEX: u32 = 1;
const DEFAULT_SIZE: usize = 65536;
const DEFAULT_ITERS: usize = 1000;
const DEFAULT_TX_DEPTH: usize = 128;
const MAX_RD_ATOMIC: u8 = 16;

pub struct Config {
    /// `None` → server mode.  `Some(host)` → client mode, connect to `host`.
    pub server: Option<String>,
    pub port: u16,
    pub gid_index: u32,
    /// Size of each RDMA READ in bytes.
    pub size: usize,
    /// Total number of RDMA READ operations to issue.
    pub iters: usize,
    /// Maximum number of in-flight RDMA READs at any one time.
    pub tx_depth: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: None,
            port: DEFAULT_PORT,
            gid_index: DEFAULT_GID_INDEX,
            size: DEFAULT_SIZE,
            iters: DEFAULT_ITERS,
            tx_depth: DEFAULT_TX_DEPTH,
        }
    }
}

pub fn run(cfg: Config) -> io::Result<()> {
    // Keep `devices` alive until after `open()`.
    let ctx = {
        ibverbs::devices()?
            .iter()
            .next()
            .ok_or_else(|| {
                io::Error::new(ErrorKind::NotFound, "no rdma device found")
            })?
            .open()?
    };

    // CQ must be large enough to hold all in-flight completions.
    // Because rust-ibverbs always uses IBV_SEND_SIGNALED (see module doc),
    // every posted READ generates one CQ entry. We size the CQ so that a
    // full tx_depth batch can complete before we drain it.
    let cq_size = (cfg.tx_depth * 2 + 1) as i32;
    let cq = ctx.create_cq(cq_size, 0)?;
    let pd = ctx.alloc_pd()?;
    // `allocate` uses DEFAULT_ACCESS_FLAGS which includes IBV_ACCESS_REMOTE_READ.
    let mut mr: LocalMemoryRegion<'_, u8> = pd.allocate(cfg.size)?;

    let max_rd_atomic = MAX_RD_ATOMIC.min(cfg.tx_depth as u8);
    let cap = ibv_qp_cap { max_send_wr: 1, max_recv_wr: 1, max_send_sge: 1, max_recv_sge: 1, max_inline_data: 0 };

    match &cfg.server.clone() {
        Some(host) => {
            let (qp, my_info) = build_qp(&cfg, &pd, &cq, &mut mr, max_rd_atomic, cap)?;
            run_client(cfg, qp, my_info, host)
        }
        None => run_server(cfg, &pd, &cq, &mut mr, max_rd_atomic, cap),
    }
}

/// Builds and prepares a fresh queue pair, along with the `PeerInfo` a peer
/// needs to connect to it. Each accepted connection gets its own queue pair,
/// since a `PreparedQueuePair` is consumed by `handshake`.
fn build_qp<'res>(
    _cfg: &Config,
    pd: &'res ibverbs::ProtectionDomain<'res>,
    cq: &'res ibverbs::CompletionQueue,
    mr: &mut LocalMemoryRegion<'_, u8>,
    max_rd_atomic: u8,
    cap: ibv_qp_cap,
) -> io::Result<(ibverbs::PreparedQueuePair<'res>, PeerInfo)> {
    let qp = pd.create_qp(cq, cq, ibv_qp_type::IBV_QPT_RC, cap)
        //.set_gid_index(cfg.gid_index)
        .allow_remote_rw()
        // Both sides set max_rd_atomic / max_dest_rd_atomic so that either
        // side could initiate RDMA operations in future bidirectional tests.
        .set_max_rd_atomic(max_rd_atomic)
        .set_max_dest_rd_atomic(max_rd_atomic)
        .build()?;

    let endpoint = qp.endpoint();
    let remote = mr.remote();
    let my_info = PeerInfo {
        qp_num: endpoint.num,
        lid: endpoint.lid,
        gid: endpoint.gid.map_or([0u8; 16], |gid| gid.raw),

        addr: remote.addr,
        rkey: remote.rkey,
    };

    Ok((qp, my_info))
}

fn run_server(
    cfg: Config,
    pd: &ibverbs::ProtectionDomain,
    cq: &ibverbs::CompletionQueue,
    mr: &mut LocalMemoryRegion<'_, u8>,
    max_rd_atomic: u8,
    cap: ibv_qp_cap,
) -> io::Result<()> {
    let listen_addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), cfg.port);
    let mut listener = TcpListener::bind(SocketAddr::V4(listen_addr)).unwrap();
    println!("Waiting for client to connect on {} ...", listen_addr);

    // Keep accepting new clients forever, one at a time, each with its own
    // freshly-built queue pair.
    loop {
        let mut stream = listener.accept().unwrap();
        println!("Client connected from {}", stream.peer_addr());

        let (prepared, my_info) = build_qp(&cfg, pd, cq, mr, max_rd_atomic, cap)?;

        // Handshake: client sends first, server responds.
        let client_info = PeerInfo::read_from(&mut stream).unwrap();
        my_info.write_to(&mut stream).unwrap();

        // Connect the QP to the client.
        let mut _qp = prepared.handshake(endpoint_from_peer(&client_info)).unwrap();
        println!("QP connected – waiting for client to finish the test ...");

        // Wait for client's end-of-test signal.
        comm::sync(&mut stream).unwrap();
        println!("Done. Waiting for next client ...");
    }
}

fn run_client(
    cfg: Config,
    prepared: ibverbs::PreparedQueuePair,
    my_info: PeerInfo,
    host: &str,
) -> io::Result<()> {
    println!("RDMA_Read BW Test");
    println!("Connecting to {}:{}", host, cfg.port);
    let mut stream = TcpStream::connect(SocketAddr::try_from((IpAddr::from_str(host).expect("cannot parse host as ip addr"), cfg.port)).unwrap())
        .expect("cannot connect");

    // Handshake: client sends first, server responds.
    my_info.write_to(&mut stream).expect("cannot write info to server");
    let server_info = PeerInfo::read_from(&mut stream).expect("cannot read info from server");

    // Connect the QP to the server.
    let _qp = prepared.handshake(endpoint_from_peer(&server_info))?;

    //let (bw_gbps, msg_rate_mpps) = bw_loop(&mut qp, &mut mr, &mut cq, &cfg)?;

    println!("{:>8}  {:>12}  {:>18.2}  {:>14.6}", "#bytes", "#iterations", "BW avg[Gb/sec]", "MsgRate[Mpps]");
    //println!("{:>8}  {:>12}  {:>18.2}  {:>14.6}", cfg.size, cfg.iters, bw_gbps, msg_rate_mpps);

    // Signal the server that the test is finished.
    comm::sync(&mut stream).expect("cannot synchronize with server");

    Ok(())
}

fn endpoint_from_peer(info: &PeerInfo) -> QueuePairEndpoint {
    let gid = if info.gid != [0u8; 16] {
        Some(Gid {raw: info.gid})
    } else {
        None
    };
    QueuePairEndpoint { num: info.qp_num, lid: info.lid, gid }
}

/*
/// Issues `cfg.iters` RDMA READs while keeping up to `cfg.tx_depth` in flight.
///
/// Returns `(bandwidth_gbps, message_rate_mpps)`.
fn bw_loop(
    qp: &mut ibverbs::QueuePair,
    mr: &mut LocalMemoryRegion<'_, u8>,
    cq: &mut CompletionQueue<'_>,
    cfg: &Config,
) -> io::Result<(f64, f64)> {
    // Pre-compute the local SGE once; LocalMemorySlice is Copy so we can
    // reuse it freely without re-borrowing the MemoryRegion.
    let local_sge: Vec<u8> = Vec::new();

    // Scratch space for polling completions.
    let mut poll_buf = vec![ibv_wc::default(); cfg.tx_depth];

    let mut posted = 0usize;
    let mut completed = 0usize;

    // Seed the pipeline with the first batch.
    let initial = cfg.tx_depth.min(cfg.iters);
    for wr_id in 0..initial {
        // NOTE: rust-ibverbs forces IBV_SEND_SIGNALED on every post_read call,
        // so every READ generates a CQ entry.  Ideally we would only signal
        // every tx_depth operations.  See module-level doc for details.
        qp.post_receive(mr, vec![local_sge], vec![wr_id as u64])?;
        posted += 1;
    }

    let t0 = Instant::now();

    // Drain completions and refill the pipeline until all iterations are done.
    while completed < cfg.iters {
        let wcs = cq.poll(&mut poll_buf)?;
        for wc in wcs.iter() {
            if let Some((status, vendor_err)) = wc.error() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("WC error: {status:?} vendor_err={vendor_err}"),
                ));
            }
            completed += 1;

            // Keep the pipeline full.
            if posted < cfg.iters {
                qp.post_receive(mr, vec![local_sge], vec![posted as u64])?;
                posted += 1;
            }
        }
    }

    let elapsed = t0.elapsed();
    let secs = elapsed.as_secs_f64();
    let bytes = cfg.iters as f64 * cfg.size as f64;
    let bw_gbps = bytes * 8.0 / secs / 1e9;
    let msg_rate_mpps = cfg.iters as f64 / secs / 1e6;

    Ok((bw_gbps, msg_rate_mpps))
}
*/
