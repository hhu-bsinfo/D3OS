use std::fs::File;
use std::io::Write;

fn main() {
    let host = std::env::var("HOST_MACHINE").unwrap_or_else(|_| "unknown".to_string());
    let source_ip = std::env::var("SOURCE_IP").unwrap_or_else(|_| "unknown".to_string());
    let target_ip = std::env::var("TARGET_IP").unwrap_or_else(|_| "unknown".to_string());
    let target_port = std::env::var("TARGET_PORT").unwrap_or_else(|_| "unknown".to_string());
    let is_sender = std::env::var("IS_SENDER").unwrap_or_else(|_| "false".to_string())
        .parse::<bool>().unwrap();

    let host_1 = "ib3";
    let host_2 = "ib4";

    let mut build_file = File::create("src/build_constants.rs").unwrap();

    writeln!(build_file, "pub const THIS_HOST: &str = {:?};", host).unwrap();
    writeln!(build_file, "pub const IS_SENDER: bool = {};", is_sender).unwrap();

    let target_host = if host == host_1 { host_2 } else { host_1 };
    writeln!(build_file, "pub const TARGET_HOST: &str = {:?};", target_host).unwrap();
    writeln!(build_file, "pub const THIS_IP: &str = {:?};", source_ip).unwrap();
    writeln!(build_file, "pub const TARGET_IP: &str = {:?};", target_ip).unwrap();
    writeln!(build_file, "pub const TARGET_PORT: &str = {:?};", target_port).unwrap();
}