use std::fs::File;
use std::io::Write;

fn main() {
    // Write build information to a file
    built::write_built_file().expect("Failed to acquire build-time information");

    let host = std::env::var("HOST_MACHINE").unwrap_or_else(|_| "unknown".to_string());
    let source_ip = std::env::var("SOURCE_IP").unwrap_or_else(|_| "unknown".to_string());
    let target_ip = std::env::var("TARGET_IP").unwrap_or_else(|_| "unknown".to_string());
    let target_port = std::env::var("TARGET_PORT").unwrap_or_else(|_| "unknown".to_string());
    let gw_ip = std::env::var("GATEWAY_IP").unwrap_or_else(|_| "unknown".to_string());

    let host_1 = "ib3";
    let host_2 = "ib4";

    let mut build_file = File::create("src/build_constants.rs").unwrap();

    writeln!(build_file, "pub const THIS_HOST: &str = {:?};", host).unwrap();

    let target_host = if host == host_1 { host_2 } else { host_1 };
    writeln!(build_file, "pub const TARGET_HOST: &str = {:?};", target_host).unwrap();
    writeln!(build_file, "pub const THIS_IP: &str = {:?};", source_ip).unwrap();
    writeln!(build_file, "pub const TARGET_IP: &str = {:?};", target_ip).unwrap();
    writeln!(build_file, "pub const TARGET_PORT: &str = {:?};", target_port).unwrap();
    writeln!(build_file, "pub const GATEWAY_IP: &str = {:?};", gw_ip).unwrap();
}