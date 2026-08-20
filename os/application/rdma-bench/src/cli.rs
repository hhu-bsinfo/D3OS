use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::iter::Peekable;
use core::net::{IpAddr, Ipv4Addr};
use network::resolve_hostname;
use runtime::env;
use runtime::env::Args;
use serde::{Deserialize, Serialize};

const USAGE: &str = concat!(
    "Usage: rdma-bench server [--port N] [--listen]\n",
    "       rdma-bench client --host HOST [--port N] [--transport rc|uc|ud]\n",
    "                         [--mode M[,M..]] [--size N[,N..]] [--min-size N] [--max-size N]\n",
    "                         [--iterations N] [--tx-depth N]\n",
    "\n",
    "--mode selects bandwidth, latency, accuracy, rdma-write and/or rdma-read; left out, all\n",
    "five run.\n",
    "--size selects explicit message sizes; left out, the client sweeps every power of two from\n",
    "--min-size to --max-size. So a client without --mode and --size runs the complete suite and\n",
    "prints one table per mode. Every (mode, size) pair opens its own connection, so unless the\n",
    "run is a single benchmark the peer must be started as `rdma-bench server --listen`."
);

const DEFAULT_PORT: u16 = 18515;
const DEFAULT_ITERATIONS: usize = 1000;
const DEFAULT_TX_DEPTH: usize = 32;

/// Bounds of the default message size sweep. The lower one is the smallest size accuracy mode can
/// identify (it needs room for its 8-byte sequence-number header); the upper one is kept at 64 KiB
/// because accuracy mode registers a `tx_depth`-slot buffer, so its memory region grows with the
/// message size — sweeping higher is fine, but pair it with a smaller `--tx-depth`.
const DEFAULT_MIN_SIZE: usize = 8;
const DEFAULT_MAX_SIZE: usize = 1 << 16;

/// Also the wire type sent as part of `BenchmarkRequest`, so it needs to be `Serialize`/
/// `Deserialize` rather than just parsed from argv.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    Rc,
    Uc,
    Ud,
}

impl Transport {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "rc" => Ok(Transport::Rc),
            "uc" => Ok(Transport::Uc),
            "ud" => Ok(Transport::Ud),
            _ => Err(format!("unknown transport '{}': expected rc, uc, or ud", s)),
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Bandwidth,
    Latency,
    Accuracy,
    /// One-sided RDMA WRITE bandwidth: the client drives a window of writes into a buffer the
    /// server merely exposes, so the server's CPU never sees a completion.
    RdmaWrite,
    /// One-sided RDMA READ bandwidth, same shape as `RdmaWrite` but pulling instead of pushing.
    /// Not supported on UC — only RC has RDMA READ in its transport-service repertoire.
    RdmaRead,
}

impl Mode {
    /// Every mode, in the order a suite runs them.
    pub const ALL: [Mode; 5] =
        [Mode::Bandwidth, Mode::Latency, Mode::Accuracy, Mode::RdmaWrite, Mode::RdmaRead];

    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "bandwidth" => Ok(Mode::Bandwidth),
            "latency" => Ok(Mode::Latency),
            "accuracy" => Ok(Mode::Accuracy),
            "rdma-write" => Ok(Mode::RdmaWrite),
            "rdma-read" => Ok(Mode::RdmaRead),
            _ => Err(format!(
                "unknown mode '{}': expected bandwidth, latency, accuracy, rdma-write, or rdma-read",
                s
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Mode::Bandwidth => "bandwidth",
            Mode::Latency => "latency",
            Mode::Accuracy => "accuracy",
            Mode::RdmaWrite => "rdma-write",
            Mode::RdmaRead => "rdma-read",
        }
    }

    /// Smallest message size this mode can be run with.
    pub fn min_msg_size(&self) -> usize {
        match self {
            // The sequence-number header a received message is identified by.
            Mode::Accuracy => 8,
            _ => 1,
        }
    }
}

pub enum Cli {
    Server(ServerArgs),
    Client(ClientArgs),
}

pub struct ServerArgs {
    pub port: u16,
    /// Keep accepting connections and serving benchmark runs one after another instead of
    /// exiting after the first.
    pub listen: bool,
}

/// The benchmark matrix to run: every mode in `modes` once per entry in `sizes`. A single
/// benchmark is just the one-by-one case of that.
pub struct ClientArgs {
    pub host: IpAddr,
    pub port: u16,
    pub transport: Transport,
    /// Modes to run, in the order given.
    pub modes: Vec<Mode>,
    /// Message sizes in bytes to run each mode at, ascending.
    pub sizes: Vec<usize>,
    /// Number of messages to exchange per run.
    pub iterations: usize,
    /// Number of sends/receives allowed to be outstanding at once.
    pub tx_depth: usize,
}

impl ClientArgs {
    /// Whether this is a single explicit benchmark rather than a sweep — the two are reported
    /// differently.
    pub fn is_single_run(&self) -> bool {
        self.modes.len() == 1 && self.sizes.len() == 1
    }
}

impl Cli {
    pub fn parse() -> Result<Cli, String> {
        let mut args = env::args().peekable();
        args.next(); // skip program name

        match args.next().as_deref() {
            Some("server") => Self::parse_server(args).map(Cli::Server),
            Some("client") => Self::parse_client(args).map(Cli::Client),
            _ => Err(USAGE.to_string()),
        }
    }

    fn parse_server(mut args: Peekable<Args>) -> Result<ServerArgs, String> {
        let mut server = ServerArgs { port: DEFAULT_PORT, listen: false };

        loop {
            match args.peek().map(String::as_str) {
                Some("--port") => {
                    let val = Self::next_value(&mut args, "--port")?;
                    server.port = val.parse().map_err(|_| "invalid --port value".to_string())?;
                }
                Some("--listen") => {
                    args.next();
                    server.listen = true;
                }
                Some(_) => return Err(USAGE.to_string()),
                None => break,
            }
        }

        Ok(server)
    }

    fn parse_client(mut args: Peekable<Args>) -> Result<ClientArgs, String> {
        let mut host: Option<IpAddr> = None;
        let mut min_size = DEFAULT_MIN_SIZE;
        let mut max_size = DEFAULT_MAX_SIZE;
        // Left as `None`, these mean "not asked for explicitly", which is what turns the run into
        // a full suite: all modes, and the whole power-of-two size sweep.
        let mut modes: Option<Vec<Mode>> = None;
        let mut sizes: Option<Vec<usize>> = None;
        let mut client = ClientArgs {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            transport: Transport::Rc,
            modes: Vec::new(),
            sizes: Vec::new(),
            iterations: DEFAULT_ITERATIONS,
            tx_depth: DEFAULT_TX_DEPTH,
        };

        loop {
            match args.peek().map(String::as_str) {
                Some("--host") => {
                    let val = Self::next_value(&mut args, "--host")?;
                    host = Some(
                        resolve_hostname(&val)
                            .into_iter()
                            .next()
                            .ok_or_else(|| "could not resolve --host".to_string())?,
                    );
                }
                Some("--port") => {
                    let val = Self::next_value(&mut args, "--port")?;
                    client.port = val.parse().map_err(|_| "invalid --port value".to_string())?;
                }
                Some("--transport") => {
                    let val = Self::next_value(&mut args, "--transport")?;
                    client.transport = Transport::parse(&val)?;
                }
                Some("--mode") => {
                    let val = Self::next_value(&mut args, "--mode")?;
                    let parsed = val.split(',').map(Mode::parse).collect::<Result<Vec<_>, _>>()?;
                    if parsed.is_empty() {
                        return Err("--mode must name at least one mode".to_string());
                    }
                    modes = Some(parsed);
                }
                Some("--size") => {
                    let val = Self::next_value(&mut args, "--size")?;
                    let mut parsed = val
                        .split(',')
                        .map(|s| s.parse::<usize>().map_err(|_| "invalid --size value".to_string()))
                        .collect::<Result<Vec<_>, _>>()?;
                    parsed.sort_unstable();
                    parsed.dedup();
                    if parsed.first() == Some(&0) {
                        return Err("--size must be greater than zero".to_string());
                    }
                    sizes = Some(parsed);
                }
                Some("--min-size") => {
                    let val = Self::next_value(&mut args, "--min-size")?;
                    min_size = val.parse().map_err(|_| "invalid --min-size value".to_string())?;
                }
                Some("--max-size") => {
                    let val = Self::next_value(&mut args, "--max-size")?;
                    max_size = val.parse().map_err(|_| "invalid --max-size value".to_string())?;
                }
                Some("--iterations") => {
                    let val = Self::next_value(&mut args, "--iterations")?;
                    client.iterations = val.parse().map_err(|_| "invalid --iterations value".to_string())?;
                }
                Some("--tx-depth") => {
                    let val = Self::next_value(&mut args, "--tx-depth")?;
                    client.tx_depth = val.parse().map_err(|_| "invalid --tx-depth value".to_string())?;
                }
                Some(_) => return Err(USAGE.to_string()),
                None => break,
            }
        }

        client.modes = modes.unwrap_or_else(|| Mode::ALL.to_vec());
        client.sizes = match sizes {
            Some(sizes) => sizes,
            None => power_of_two_sizes(min_size, max_size)?,
        };
        if client.iterations == 0 {
            return Err("--iterations must be greater than zero".to_string());
        }

        client.host = host.ok_or_else(|| "client mode requires --host".to_string())?;
        Ok(client)
    }

    /// Consumes the current flag and returns the value that follows it.
    fn next_value(args: &mut Peekable<Args>, option_name: &str) -> Result<String, String> {
        args.next();
        args.next().ok_or_else(|| format!("missing value for option {}", option_name))
    }
}

/// Powers of two from the first one at or above `min` up to the last one at or below `max`.
/// Non-power-of-two bounds are rounded inwards, so `4000..=100000` sweeps 4096..=65536.
fn power_of_two_sizes(min: usize, max: usize) -> Result<Vec<usize>, String> {
    if min == 0 {
        return Err("--min-size must be greater than zero".to_string());
    }
    if max < min {
        return Err("--max-size must not be smaller than --min-size".to_string());
    }

    let mut sizes = Vec::new();
    let mut size = min.next_power_of_two();
    while size <= max {
        sizes.push(size);
        match size.checked_mul(2) {
            Some(next) => size = next,
            None => break,
        }
    }

    if sizes.is_empty() {
        return Err(format!("no power-of-two message size lies between {} and {}", min, max));
    }
    Ok(sizes)
}
