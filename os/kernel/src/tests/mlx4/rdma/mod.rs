pub mod rdma_read;
pub mod rdma_write;
mod handshake;
pub mod session;
mod integrity;

use integrity::{MAGIC_HEADER, CHECKSUM_SIZE, build_packet, build_payload};
use crate::infiniband::ibverbs::{
    LocalMemoryRegion
};
use session::RdmaSession;
use alloc::{vec, vec::Vec};

pub const ALLOC_MEM_XS: usize = 1000;
pub const ALLOC_MEM_S: usize = 10000;
pub const ALLOC_MEM_M: usize = 100000;
pub const ALLOC_MEM_L: usize = 1000000;
pub const ALLOC_MEM_XL: usize = 10000000;
pub const ALLOC_MEM_XXL: usize= 40000000;
pub const ALLOC_MEM_XXXL: usize = 1000000000;

pub const ALLOC_MEM: usize = ALLOC_MEM_M;

pub const CONTEXT_BUFFER_SIZE: usize = ALLOC_MEM;
pub const PAYLOAD_FILL: u8 = 0xFA;
pub const META_DATA_SIZE: usize = MAGIC_HEADER.len() + CHECKSUM_SIZE;

pub(super) fn hit_wo_fault<F>(packet: &[u8], context_buffer: &mut [u8], f: F) 
where F: Fn(usize) -> u8 
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