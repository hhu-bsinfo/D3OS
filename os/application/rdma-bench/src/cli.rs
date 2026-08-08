use alloc::format;
use alloc::string::{String, ToString};
use core::iter::Peekable;
use core::net::{IpAddr, Ipv4Addr};
use network::resolve_hostname;
use runtime::env;
use runtime::env::Args;
use serde::{Deserialize, Serialize};

const USAGE: &str = "Usage: rdma-bench server [--port N] [--listen]\n       rdma-bench client --host HOST [--port N] [--transport rc|uc|ud] [--mode bandwidth|latency|accuracy] [--size N] [--iterations N] [--tx-depth N]";

const DEFAULT_PORT: u16 = 18515;
const DEFAULT_SIZE: usize = 65536;
const DEFAULT_ITERATIONS: usize = 1000;
const DEFAULT_TX_DEPTH: usize = 32;

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
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "bandwidth" => Ok(Mode::Bandwidth),
            "latency" => Ok(Mode::Latency),
            "accuracy" => Ok(Mode::Accuracy),
            _ => Err(format!("unknown mode '{}': expected bandwidth, latency, or accuracy", s)),
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

pub struct ClientArgs {
    pub host: IpAddr,
    pub port: u16,
    pub transport: Transport,
    pub mode: Mode,
    /// Message size in bytes for each send/receive.
    pub size: usize,
    /// Number of messages to exchange.
    pub iterations: usize,
    /// Number of sends/receives allowed to be outstanding at once.
    pub tx_depth: usize,
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
        let mut client = ClientArgs {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            transport: Transport::Rc,
            mode: Mode::Bandwidth,
            size: DEFAULT_SIZE,
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
                    client.mode = Mode::parse(&val)?;
                }
                Some("--size") => {
                    let val = Self::next_value(&mut args, "--size")?;
                    client.size = val.parse().map_err(|_| "invalid --size value".to_string())?;
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

        client.host = host.ok_or_else(|| "client mode requires --host".to_string())?;
        Ok(client)
    }

    /// Consumes the current flag and returns the value that follows it.
    fn next_value(args: &mut Peekable<Args>, option_name: &str) -> Result<String, String> {
        args.next();
        args.next().ok_or_else(|| format!("missing value for option {}", option_name))
    }
}
