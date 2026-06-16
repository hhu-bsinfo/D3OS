#![no_std]

extern crate alloc;
mod cli;
mod client;
mod protocol;
mod server;
mod stats;

use crate::protocol::Coordinator;
use crate::stats::StatsTracker;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use cli::{Cli, Mode, Protocol};
use client::Client;
use concurrent::thread;
use core::fmt::{self, Display, Formatter};
use core::net::SocketAddr;
use core::sync::atomic::{AtomicBool, Ordering};
use log::info;
use network::{NetworkError, TcpListener, TcpStream, UdpSocket};
use protocol::{TCP_RECV_BUFFER_SIZE, TCP_SEND_MESSAGE_SIZE, UDP_RECV_BUFFER_SIZE, UDP_SEND_MESSAGE_SIZE};
use server::Server;
use spin::Mutex;
use stats::Stats;
use terminal::println;

// Static work queues for passing data to threads
static TCP_WORK_QUEUE: Mutex<VecDeque<TcpWorkItem>> = Mutex::new(VecDeque::new());
static UDP_SENDER_WORK_QUEUE: Mutex<VecDeque<UdpSenderWorkItem>> = Mutex::new(VecDeque::new());
static UDP_RECEIVER_WORK_QUEUE: Mutex<VecDeque<UdpReceiverWorkItem>> = Mutex::new(VecDeque::new());

/// Used to pass data to TCP worker threads
struct TcpWorkItem {
    role: Role,
    stats: Arc<Stats>,
    socket: TcpStream,
    start_flag: Arc<AtomicBool>,
    bandwidth: Option<u64>,
}

/// Used to pass data to UDP sender worker threads
struct UdpSenderWorkItem {
    stats: Arc<Stats>,
    socket: UdpSocket,
    remote_addr: SocketAddr,
    start_flag: Arc<AtomicBool>,
    bandwidth: Option<u64>,
}

/// Used to pass data to UDP receiver worker threads
struct UdpReceiverWorkItem {
    stats: Arc<Stats>,
    socket: UdpSocket,
    start_flag: Arc<AtomicBool>,
}

#[derive(Copy, Clone)]
enum Role {
    Sender,
    Receiver,
}

impl Role {
    fn inverse(self) -> Self {
        match self {
            Self::Sender => Self::Receiver,
            Self::Receiver => Self::Sender,
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Role::Sender => write!(f, "Sender"),
            Role::Receiver => write!(f, "Receiver"),
        }
    }
}

struct Results {
    pub summary: String,
    pub json: String,
}

impl Results {
    fn new(stats: &Stats) -> Self {
        Self {
            summary: stats.finalize_and_get_summary(),
            json: stats.as_json(),
        }
    }
}

/// Minimum sleep duration in milliseconds for the pacer
const MIN_SLEEP_MS: usize = 6;

/// Pacer for bandwidth limiting
struct Pacer {
    start_time: f64,
    bytes_sent: u64,
    rate_bps: u64,
}

impl Pacer {
    fn new(rate_bps: u64) -> Self {
        Self {
            start_time: time::systime().as_seconds_f64(),
            bytes_sent: 0,
            rate_bps,
        }
    }

    /// Blocks the current thread if sending new_bytes exceeds the bandwidth limit
    fn block_if_needed(&mut self, new_bytes: usize) {
        self.bytes_sent += new_bytes as u64;

        // Calculate how much time *should* have passed to send this amount of data
        let target_duration = (self.bytes_sent * 8) as f64 / self.rate_bps as f64;
        let now = time::systime().as_seconds_f64();
        let current_duration = now - self.start_time;

        if current_duration < target_duration {
            // We are too fast (ahead of schedule). Sleep to slow down.
            let sleep_seconds = target_duration - current_duration;
            let sleep_ms = (sleep_seconds * 1000.0) as usize;

            if sleep_ms >= MIN_SLEEP_MS {
                thread::sleep(sleep_ms);
            }
        } else {
            // We are too slow (behind schedule)
            // If we don't adjust, we will burst later to catch up.
            // Fix: Reset the baseline. We enforce the limit from the current moment forward.
            self.start_time = now - target_duration;
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    terminal::init_logger();
    let cli = Cli::parse();

    if let Err(message) = cli {
        println!("{}", message);
        return;
    }

    let cli = cli.unwrap();

    match cli.mode {
        Mode::Server => start_server(cli),
        Mode::Client => start_client(cli),
    }
}

/// Starts the benchmark in server mode, handles only one client at a time
fn start_server(config: Cli) {
    loop {
        let server = Server::listen(config).expect("server err");
        let client_config = server.handshake();
        let local_addr = SocketAddr::new(config.host, config.port);

        let role = if client_config.reverse { Role::Sender } else { Role::Receiver };

        let results = match client_config.protocol {
            Protocol::Tcp => {
                if client_config.parallel_streams > 1 {
                    run_tcp_parallel(&server, role, client_config, local_addr)
                } else {
                    let listener = TcpListener::bind(local_addr).expect("failed to bind tcp socket");
                    let socket = listener.accept().expect("failed to accept tcp connection");

                    // Synchronize start so that the receiver is ready before the sender starts
                    match role {
                        Role::Receiver => server.signal_ready(),
                        Role::Sender => server.wait_for_ready(),
                    }

                    run_tcp_single(role, socket, client_config)
                }
            }
            Protocol::Udp => {
                if client_config.parallel_streams > 1 {
                    run_udp_parallel(&server, role, client_config, local_addr)
                } else {
                    let remote = match role {
                        Role::Sender => Some(server.remote_addr()),
                        Role::Receiver => None,
                    };

                    // Synchronize start so that the receiver is ready before the sender starts
                    match role {
                        Role::Receiver => server.signal_ready(),
                        Role::Sender => server.wait_for_ready(),
                    }

                    run_udp(role, local_addr, remote, client_config)
                }
            }
        };

        println!("{}:", role);
        println!("{}", results.summary);

        server.send_results(results);
    }
}

/// Starts the benchmark in client mode
fn start_client(config: Cli) {
    let client = Client::connect(config).expect("client err");
    client.handshake(config);

    let role = if config.reverse { Role::Receiver } else { Role::Sender };

    let results = match config.protocol {
        Protocol::Tcp => {
            if config.parallel_streams > 1 {
                run_tcp_parallel(&client, role, config, SocketAddr::new(config.host, config.port))
            } else {
                let socket = TcpStream::connect(SocketAddr::new(config.host, config.port)).expect("failed to connect to tcp socket");

                // Synchronize start so that the receiver is ready before the sender starts
                match role {
                    Role::Receiver => client.signal_ready(),
                    Role::Sender => client.wait_for_ready(),
                }

                run_tcp_single(role, socket, config)
            }
        }
        Protocol::Udp => {
            if config.parallel_streams > 1 {
                run_udp_parallel(&client, role, config, client.local_addr())
            } else {
                let local = client.local_addr();
                let remote = match role {
                    Role::Sender => Some(SocketAddr::new(config.host, config.port)),
                    Role::Receiver => None,
                };

                // Synchronize start so that the receiver is ready before the sender starts
                match role {
                    Role::Receiver => client.signal_ready(),
                    Role::Sender => client.wait_for_ready(),
                }

                run_udp(role, local, remote, config)
            }
        }
    };

    println!("{}:", role);
    println!("{}", results.summary);

    let server_results = client.receive_server_results();

    println!("{}:", role.inverse());
    println!("{}", server_results.summary);

    if config.json_output {
        // Log JSON results to serial output
        info!("\n{}\n", results.json);
        info!("----------------------------------------");
        info!("\n{}\n", server_results.json);
    }
}

/// Runs a single TCP stream benchmark
fn run_tcp_single(role: Role, socket: TcpStream, config: Cli) -> Results {
    let stats = Stats::tcp(config.interval_seconds, config.duration_seconds, config.transfer_bytes);
    let tracker = stats.register_thread(current_thread_id());
    println!("{}", stats.get_header());

    match role {
        Role::Sender => tcp_sender_loop(&stats, tracker, socket, config.bandwidth),
        Role::Receiver => tcp_receiver_loop(&stats, tracker, socket),
    }

    Results::new(&stats)
}

/// Runs a parallel TCP stream benchmark
fn run_tcp_parallel<C: Coordinator>(coordinator: &C, role: Role, config: Cli, local_addr: SocketAddr) -> Results {
    let stats = Arc::new(Stats::tcp(config.interval_seconds, config.duration_seconds, config.transfer_bytes));
    let start_flag = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();

    let sockets = if coordinator.is_server() {
        // Only the server listens for incoming connections, preventing NAT/firewall issues
        accept_tcp_streams(coordinator, config, local_addr)
    } else {
        connect_tcp_streams(coordinator, config)
    };

    // Enqueue work items and spawn threads
    for socket in sockets {
        TCP_WORK_QUEUE.lock().push_back(TcpWorkItem {
            role,
            stats: Arc::clone(&stats),
            socket,
            start_flag: Arc::clone(&start_flag),
            bandwidth: config.bandwidth,
        });

        if let Some(t) = thread::create(tcp_thread_entry) {
            threads.push(t);
        }
    }

    // Synchronize start so that the receiver's worker threads are ready before the sender starts
    match role {
        Role::Sender => coordinator.wait_for_start_benchmark(),
        Role::Receiver => coordinator.signal_start_benchmark(),
    }

    // Unblock all threads
    start_flag.store(true, Ordering::Release);

    // Wait for all threads to finish
    join_threads(threads);

    Results::new(&stats)
}

fn connect_tcp_streams<C: Coordinator>(coordinator: &C, config: Cli) -> Vec<TcpStream> {
    let mut sockets = Vec::with_capacity(config.parallel_streams as usize);

    for _ in 0..config.parallel_streams {
        // Wait until the server is ready to accept the next stream
        coordinator.wait_for_stream_ready();
        let socket = TcpStream::connect(SocketAddr::new(config.host, config.port)).expect("failed to connect tcp stream");
        sockets.push(socket);
    }

    sockets
}

fn accept_tcp_streams<C: Coordinator>(coordinator: &C, config: Cli, local_addr: SocketAddr) -> Vec<TcpStream> {
    let mut sockets = Vec::with_capacity(config.parallel_streams as usize);

    for stream_id in 0..config.parallel_streams {
        let listener = TcpListener::bind(local_addr).expect("failed to bind tcp socket");
        // Signal the client that the server is ready to accept the next stream
        coordinator.signal_stream_ready(stream_id);
        let socket = listener.accept().expect("failed to accept tcp connection");
        sockets.push(socket);
    }

    sockets
}

fn tcp_thread_entry() {
    let work_item = TCP_WORK_QUEUE.lock().pop_front().expect("no tcp work item");
    let tracker = work_item.stats.register_thread(current_thread_id());

    wait_for_start(&work_item.start_flag);

    match work_item.role {
        Role::Sender => tcp_sender_loop(&work_item.stats, tracker, work_item.socket, work_item.bandwidth),
        Role::Receiver => tcp_receiver_loop(&work_item.stats, tracker, work_item.socket),
    }
}

/// Main execution loop for TCP receiver threads
fn tcp_receiver_loop(stats: &Stats, tracker: Arc<Mutex<StatsTracker>>, socket: TcpStream) {
    let mut buf = vec![0; TCP_RECV_BUFFER_SIZE];

    while !stats.is_finished() {
        if let Ok(true) = socket.can_recv() {
            match socket.read(&mut buf) {
                Ok(len) => {
                    if len > 0 {
                        tracker.lock().update(len, &buf);
                    }
                }
                Err(err) => {
                    if !handle_network_error(err, "receive message") {
                        break;
                    }
                }
            }
        } else {
            // Sleep briefly to avoid busy waiting
            thread::sleep(30);
        }

        stats.print_interval_report();
    }
}

/// Main execution loop for TCP sender threads
fn tcp_sender_loop(stats: &Stats, tracker: Arc<Mutex<StatsTracker>>, socket: TcpStream, bandwidth: Option<u64>) {
    let message = vec![0; TCP_SEND_MESSAGE_SIZE];
    let mut pacer = bandwidth.map(|b| Pacer::new(b));

    while !stats.is_finished() {
        if let Ok(true) = socket.can_send() {
            match socket.write(&message) {
                Ok(len) => {
                    // Check if we need to throttle to reach the bandwidth limit
                    if let Some(p) = &mut pacer {
                        p.block_if_needed(len);
                    }

                    // Pass an empty buffer
                    tracker.lock().update(len, &[]);
                },
                Err(err) => {
                    if !handle_network_error(err, "send message") {
                        break;
                    }
                }
            };
        } else {
            // Sleep briefly to avoid busy waiting
            thread::sleep(30)
        }

        stats.print_interval_report();
    }
}

fn run_udp(role: Role, local_addr: SocketAddr, remote: Option<SocketAddr>, config: Cli) -> Results {
    match role {
        Role::Sender => start_udp_sender(local_addr, remote.expect("remote addr required for UDP sender"), config),
        Role::Receiver => start_udp_receiver(local_addr, config),
    }
}

/// Runs a parallel UDP stream benchmark
fn run_udp_parallel<C: Coordinator>(coordinator: &C, role: Role, config: Cli, local_addr: SocketAddr) -> Results {
    let stats = Arc::new(Stats::udp(config.interval_seconds, config.duration_seconds, role, config.transfer_bytes));
    let start_flag = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();

    match role {
        Role::Sender => {
            let remote_ip = coordinator.remote_addr().ip();
            let remote_base_port = coordinator.remote_addr().port();

            for i in 0..config.parallel_streams {
                let socket = UdpSocket::bind(local_addr).expect("failed to bind udp socket");

                // Match the port incrementing logic used by the receiver so that each sender targets a unique receiver port
                let target_addr = SocketAddr::new(remote_ip, remote_base_port + i as u16);

                UDP_SENDER_WORK_QUEUE.lock().push_back(UdpSenderWorkItem {
                    stats: Arc::clone(&stats),
                    socket,
                    remote_addr: target_addr,
                    start_flag: Arc::clone(&start_flag),
                    bandwidth: config.bandwidth,
                });

                if let Some(t) = thread::create(udp_sender_thread_entry) {
                    threads.push(t);
                }
            }

            // Synchronize start so that the receiver's worker threads are ready before the sender starts
            coordinator.wait_for_start_benchmark();
        }
        Role::Receiver => {
            let base_port = local_addr.port();
            let ip = local_addr.ip();

            for i in 0..config.parallel_streams {
                // Increment port as you cannot reuse the same port for many UDP sockets
                let thread_port = base_port + i as u16;
                let thread_addr = SocketAddr::new(ip, thread_port);

                let socket = UdpSocket::bind(thread_addr).expect("failed to bind unique udp port");

                UDP_RECEIVER_WORK_QUEUE.lock().push_back(UdpReceiverWorkItem {
                    stats: Arc::clone(&stats),
                    socket,
                    start_flag: Arc::clone(&start_flag),
                });

                if let Some(t) = thread::create(udp_receiver_thread_entry) {
                    threads.push(t);
                }
            }

            // Allow the sender to start now that all receiver threads are ready
            coordinator.signal_start_benchmark();
        }
    }

    // Unblock all threads
    start_flag.store(true, Ordering::Release);

    // Wait for all threads to finish
    join_threads(threads);

    Results::new(&stats)
}

fn udp_receiver_thread_entry() {
    let work_item = UDP_RECEIVER_WORK_QUEUE.lock().pop_front().expect("no udp receiver work item");
    let tracker = work_item.stats.register_thread(current_thread_id());

    wait_for_start(&work_item.start_flag);
    udp_receiver_loop(&work_item.stats, tracker, work_item.socket);
}

fn udp_sender_thread_entry() {
    let work_item = UDP_SENDER_WORK_QUEUE.lock().pop_front().expect("no udp sender work item");
    let tracker = work_item.stats.register_thread(current_thread_id());

    wait_for_start(&work_item.start_flag);
    udp_sender_loop(&work_item.stats, tracker, work_item.socket, work_item.remote_addr, work_item.bandwidth);
}

fn start_udp_receiver(local_addr: SocketAddr, config: Cli) -> Results {
    let socket = UdpSocket::bind(local_addr).expect("failed to open socket");
    let stats = Stats::udp(config.interval_seconds, config.duration_seconds, Role::Receiver, config.transfer_bytes);
    let tracker = stats.register_thread(current_thread_id());
    println!("{}", stats.get_header());

    udp_receiver_loop(&stats, tracker, socket);

    Results::new(&stats)
}

fn start_udp_sender(local_addr: SocketAddr, remote_addr: SocketAddr, config: Cli) -> Results {
    let socket = UdpSocket::bind(local_addr).expect("failed to open socket");
    let stats = Stats::udp(config.interval_seconds, config.duration_seconds, Role::Sender, config.transfer_bytes);
    let tracker = stats.register_thread(current_thread_id());
    println!("{}", stats.get_header());

    udp_sender_loop(&stats, tracker, socket, remote_addr, config.bandwidth);

    Results::new(&stats)
}

/// Main execution loop for UDP receiver threads
fn udp_receiver_loop(stats: &Stats, tracker: Arc<Mutex<StatsTracker>>, socket: UdpSocket) {
    let mut buf = vec![0; UDP_RECV_BUFFER_SIZE];

    while !stats.is_finished() {
        if let Ok(true) = socket.can_recv() {
            match socket.recv_from(&mut buf) {
                Ok((len, _addr)) => {
                    if len > 0 {
                        tracker.lock().update(len, &buf);
                    }
                }
                Err(err) => {
                    if !handle_network_error(err, "receive message") {
                        break;
                    }
                }
            }
        } else {
            thread::sleep(30)
        }

        stats.print_interval_report();
    }
}

/// Main execution loop for UDP sender threads
fn udp_sender_loop(stats: &Stats, tracker: Arc<Mutex<StatsTracker>>, socket: UdpSocket, remote_addr: SocketAddr, bandwidth: Option<u64>) {
    let mut message = vec![0; UDP_SEND_MESSAGE_SIZE];
    let mut seq_num: u64 = 0;

    let mut pacer = bandwidth.map(|b| Pacer::new(b));

    while !stats.is_finished() {
        if let Ok(true) = socket.can_send() {
            message[..8].copy_from_slice(&seq_num.to_le_bytes());
            message[8..16].copy_from_slice(&time::systime().as_seconds_f64().to_le_bytes());

            match socket.send_to(&message, remote_addr) {
                Ok(len) => {
                    if let Some(p) = &mut pacer {
                        p.block_if_needed(len);
                    }

                    tracker.lock().update(len, &[]);
                    seq_num += 1;
                }
                Err(err) => {
                    if !handle_network_error(err, "send message") {
                        break;
                    }
                }
            };
        } else {
            thread::sleep(30)
        }

        stats.print_interval_report();
    }
}

fn current_thread_id() -> usize {
    thread::current().map(|t| t.id()).unwrap_or(0)
}

/// Blocks until the flag is set to true
fn wait_for_start(flag: &AtomicBool) {
    while !flag.load(Ordering::Acquire) {
        thread::switch();
    }
}

fn join_threads(threads: Vec<thread::Thread>) {
    for t in threads {
        match t.join() {
            Ok(_) => {}
            // Sometimes getting an error (ESRCH), ignore it
            Err(_) => {}
        }
    }
}

fn handle_network_error(err: NetworkError, operation: &str) -> bool {
    match err {
        NetworkError::DeviceBusy => true,
        NetworkError::InvalidAddress => {
            println!("Failed to {}: Invalid address.", operation);
            false
        }
        NetworkError::Unknown(_) => {
            println!("Failed to {}.", operation);
            false
        }
    }
}
