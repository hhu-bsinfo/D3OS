use crate::Results;
use crate::cli::Cli;
use crate::protocol::{ControlMsg, Coordinator, recv_msg, send_msg};
use core::net::SocketAddr;
use network::{NetworkError, TcpListener, TcpStream};
use terminal::println;

/// Wrapper around the server-side control channel.
pub struct Server {
    control_channel: TcpStream,
}

impl Server {
    /// Blocks until a client connects to the specified host and port.
    pub fn listen(config: Cli) -> Result<Server, NetworkError> {
        let listener = TcpListener::bind(SocketAddr::new(config.host, config.port))?;

        println!("-------------------------------------------");
        println!("Server listening on {}, port {}", config.host, config.port);
        println!("-------------------------------------------");

        let control_channel = listener.accept()?;

        println!(
            "Accepted connection from {} port {}",
            control_channel.peer_addr().ip(),
            control_channel.peer_addr().port()
        );

        Ok(Server { control_channel })
    }

    /// Performs the handshake with the connected client and returns the client's CLI configuration.
    pub fn handshake(&self) -> Cli {
        let client_arguments = match recv_msg(&self.control_channel) {
            ControlMsg::CliArgs(cli) => cli,
            _ => panic!("wrong control message"),
        };

        send_msg(&self.control_channel, &ControlMsg::Ack);

        client_arguments
    }

    /// Sends the results of the benchmark to the connected client.
    pub fn send_results(&self, results: Results) {
        send_msg(&self.control_channel, &ControlMsg::Results(results.summary, results.json));
    }
}

impl Coordinator for Server {
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
        true
    }
}
