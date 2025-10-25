use crc32fast::Hasher;
use alloc::{vec, vec::Vec};
use super::PAYLOAD_FILL;

pub const MAGIC_HEADER: [u8; 10] = [0x44, 0x33, 0x4F, 0x53, 0x2D, 0x52, 0x44, 0x4D, 0x41, 0x00];
pub const CHECKSUM_SIZE: usize = (u32::BITS / 8) as usize;

#[derive(Debug)]
pub enum IntegrityError {
    BadMagic,
    ChecksumMismatch,
    PacketTooSmall,
}

pub fn build_checksum(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

pub fn contains_magic(data: &[u8]) -> Result<(), IntegrityError> {
    if data.len() < MAGIC_HEADER.len() {
        return Err(IntegrityError::PacketTooSmall);
    }
    if &data[..MAGIC_HEADER.len()] != MAGIC_HEADER {
        Err(IntegrityError::BadMagic)
    } else {
        Ok(())
    }
}

pub fn matches_checksum(payload: &[u8], expected: u32) -> Result<(), IntegrityError> {
    let calculated = build_checksum(payload);
    if calculated == expected {
        Ok(())
    } else {
        Err(IntegrityError::ChecksumMismatch)
    }
}

pub fn validate_packet(packet: &[u8]) -> Result<&[u8], IntegrityError> {
    contains_magic(packet)?;

    if packet.len() < MAGIC_HEADER.len() + CHECKSUM_SIZE {
        return Err(IntegrityError::PacketTooSmall);
    }

    let payload_end = packet.len() - CHECKSUM_SIZE;
    let payload = &packet[MAGIC_HEADER.len()..payload_end];

    let expected_checksum = u32::from_be_bytes(packet[payload_end..].try_into().unwrap());

    matches_checksum(payload, expected_checksum)?;
    Ok(payload)
}

pub fn build_packet(
    payload: &[u8],
    buffer: &mut [u8],
) -> Result<usize, IntegrityError> {
    let total_len = MAGIC_HEADER.len() + payload.len() + CHECKSUM_SIZE;
    if buffer.len() < total_len {
        return Err(IntegrityError::PacketTooSmall);
    }

    buffer[..MAGIC_HEADER.len()].copy_from_slice(&MAGIC_HEADER);

    buffer[MAGIC_HEADER.len()..MAGIC_HEADER.len() + payload.len()].copy_from_slice(payload);

    let checksum = build_checksum(payload);
    buffer[MAGIC_HEADER.len() + payload.len()..total_len].copy_from_slice(&checksum.to_be_bytes());

    Ok(total_len)
}

pub fn build_payload<F>(
    payload_size: usize,
    pattern: F,
) -> Vec<u8>
where
    F: Fn(usize) -> u8,
{
    let mut buf = vec![0u8; payload_size];

    for i in 0..payload_size {
        buf[i] = pattern(i);
    }

    buf
}

pub struct pattern_functions {
    pub xor: fn(usize) -> u8,
    pub seq: fn(usize) -> u8,
    pub rot: fn(usize) -> u8,
    pub lcg: fn(usize) -> u8,
    pub fill: fn(usize) -> u8
}

pub const PAYLOAD_FUNCTIONS: pattern_functions = pattern_functions {
    xor: pattern_xor_index,
    seq: pattern_sequential,
    rot: pattern_mix_rotate,
    lcg: pattern_glibc_lcg,
    fill: pattern_fill
};

fn pattern_fill(_: usize) -> u8 {
    PAYLOAD_FILL
}

fn pattern_xor_index(i: usize) -> u8 {
    let b0 = (i & 0xFF) as u8;
    let b1 = ((i >> 8) & 0xFF) as u8;
    let b2 = ((i >> 16) & 0xFF) as u8;
    b0 ^ b1 ^ b2
}

fn pattern_sequential(i: usize) -> u8 {
    (i & 0xFF) as u8
}

fn pattern_mix_rotate(i: usize) -> u8 {
    let mut x = i as u32;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 13;
    x = x.wrapping_mul(0xc2b2_ae35);
    ((x >> ((i & 3) as u32 * 8)) & 0xFF) as u8
}

fn pattern_glibc_lcg(i: usize) -> u8 {
    let a: u32 = 1103515245;
    let c: u32 = 12345;
    let m: u32 = 0x7FFFFFFF;

    let x = ((a.wrapping_mul(i as u32).wrapping_add(c)) & m) as u32;

    (x & 0xFF) as u8
}
