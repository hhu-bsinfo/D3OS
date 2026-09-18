#![feature(slice_index_methods)]
#![no_std]

mod bench;
mod handshake;
mod integrity;
mod rdma_read;
mod rdma_write;
mod session;

extern crate alloc;

use core::net::Ipv4Addr;

use alloc::{string::String, vec::Vec};
use integrity::{CHECKSUM_SIZE, MAGIC_HEADER, build_packet, build_payload};

use runtime::{env::Args, *};
use terminal::{print, println};

use crate::bench::Benchmark;

pub const ALLOC_MEM_XS: usize = 1000;
pub const ALLOC_MEM_S: usize = 10000;
pub const ALLOC_MEM_M: usize = 100000;
pub const ALLOC_MEM_L: usize = 1000000;
pub const ALLOC_MEM_XL: usize = 10000000;
pub const ALLOC_MEM_XXL: usize = 40000000;
pub const ALLOC_MEM_XXXL: usize = 1000000000;

pub const ALLOC_MEM: usize = ALLOC_MEM_XL;

pub const CONTEXT_BUFFER_SIZE: usize = ALLOC_MEM;
pub const PAYLOAD_FILL: u8 = 0xFA;
pub const META_DATA_SIZE: usize = MAGIC_HEADER.len() + CHECKSUM_SIZE;

pub fn hit_wo_fault<F>(packet: &[u8], context_buffer: &mut [u8], f: F)
where
    F: Fn(usize) -> u8,
{
    let payload = build_payload(ALLOC_MEM - META_DATA_SIZE, f);

    let expected_packet_len = build_packet(&payload[..], context_buffer).expect("failed to create packet");
    let expected_packet = &context_buffer[..expected_packet_len];

    let mut total_correct_bytes = 0u64;

    for (b, &expected) in packet.iter().zip(expected_packet.iter()) {
        if *b == expected {
            total_correct_bytes += 1;
        }
    }

    let hit_rate = ((total_correct_bytes as f64) / (ALLOC_MEM as f64)) * 100.0;

    println!("hit rate: {:.2}%", hit_rate);
}

#[derive(Debug)]
enum RdamType {
    Read,
    Write,
}

#[derive(Debug)]
struct RunConfig {
    target_ip: Ipv4Addr,
    target_port: u16,
    benchmark: Benchmark,
    rdma_type: RdamType,
    only_test: bool,
    is_sender: bool,
}

impl RunConfig {
    fn parse_args(args: Args) -> Option<Self>{
        let args: Vec<String> = args.collect();

        if args.len() != 3 {
            return None;
        }

        let config = Self {
            target_ip: args[1].parse().ok()?,
            target_port: args[2].parse().ok()?,
            benchmark: Benchmark::Throughput,
            rdma_type: RdamType::Read,
            only_test: false,
            is_sender: false,
        };

        for (i, arg) in args.iter().enumerate() {
            println!("{}: {}", i, arg)
        }

        Some(config)
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let config = match RunConfig::parse_args(env::args()) {
        Some(config) => config,
        None => {
            println!("USAGE: rdma_test_mlx4 [OPTIONS] TARGET_IP TARGET_PORT");
            return;
        },
    };

    print!("{:?}", config);

    /*
    match config.rdma_type {
        RdamType::Read => rdma_read::invoke(config),
        RdamType::Write => rdma_write::invoke(config),
    }
    */
}
