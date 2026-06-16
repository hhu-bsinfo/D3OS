use crate::Results;
use crate::cli::Cli;
use crate::protocol::{ControlMsg, Coordinator, recv_msg, send_msg};
use core::net::SocketAddr;
use network::{NetworkError, TcpStream};
use terminal::println;

/// Wrapper around the client-side control channel.
pub struct Client {
    control_channel: TcpStream,
}

impl Client {
    /// Connects to the server at the specified host and port in the CLI configuration.
    pub fn connect(config: Cli) -> Result<Client, NetworkError> {
        let control_channel = TcpStream::connect(SocketAddr::new(config.host, config.port))?;

        println!("-------------------------------------------");
        println!("Connected to {} on port {}", config.host, config.port);
        println!("-------------------------------------------");

        Ok(Client { control_channel })
    }

    /// Performs the handshake with the server, sending the CLI configuration and waiting for acknowledgment.
    pub fn handshake(&self, config: Cli) {
        send_msg(&self.control_channel, &ControlMsg::CliArgs(config));

        match recv_msg(&self.control_channel) {
            ControlMsg::Ack => {}
            _ => panic!("handshake failed"),
        }
    }

    /// Receives the server's results of the benchmark.
    pub fn receive_server_results(&self) -> Results {
        match recv_msg(&self.control_channel) {
            ControlMsg::Results(summary, json) => Results { summary, json },
            _ => panic!("expected results from server"),
        }
    }
}

impl Coordinator for Client {
    fn send(&self, msg: &ControlMsg) {
        send_msg(&self.control_channel, msg);
    }

    fn recv(&self) -> ControlMsg {
        recv_msg(&self.control_channel)
    }

    fn local_addr(&self) -> SocketAddr {
        self.control_channel.local_addr()
    }

    fn remote_addr(&self) -> SocketAddr {
        self.control_channel.peer_addr()
    }

    fn is_server(&self) -> bool {
        false
    }
}
