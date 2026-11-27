//! Shared D3OS keyboard syscall and decoded-key frame formats.

use crate::{DecodedKey, KeyCode, KeyboardInput};
use stream::{event_from_u16, event_to_u16};

pub const READ_RAW: usize = 0;
pub const READ_UNICODE: usize = 1;
pub const READ_EVENT: usize = 2;
pub const GET_LAYOUT: usize = 3;
pub const SET_LAYOUT: usize = 4;

// Zero means no decoded key, 1..=0x110000 encode Unicode scalars, and the
// remaining values encode non-character KeyCodes.
const RAW_KEY_BASE: u32 = 0x11_0001;

pub fn encode_key(key: DecodedKey) -> u32 {
    match key {
        DecodedKey::Unicode(character) => character as u32 + 1,
        DecodedKey::RawKey(code) => RAW_KEY_BASE + code as u32,
    }
}

pub fn decode_key(value: u32) -> Option<DecodedKey> {
    match value {
        0 => None,
        1..=0x11_0000 => char::from_u32(value - 1).map(DecodedKey::Unicode),
        _ => {
            let code = value - RAW_KEY_BASE;
            if code > KeyCode::Unknown as u32 {
                return None;
            }
            // pc-keyboard 0.9 KeyCode is a contiguous repr(u8) enum.
            Some(DecodedKey::RawKey(event_from_u16(code as u16).code))
        }
    }
}

pub fn encode_event(input: KeyboardInput) -> u64 {
    // Add one so zero remains reserved for an empty queue even if a raw
    // event and its decoded representation both have zero-valued fields.
    let raw = u64::from(event_to_u16(input.raw));
    let decoded = u64::from(input.decoded.map_or(0, encode_key));
    ((decoded << 16) | raw) + 1
}

pub fn decode_raw(raw: u16) -> Option<crate::KeyEvent> {
    if u32::from(raw & 0xff) > KeyCode::Unknown as u32 || raw >> 8 > 2 {
        return None;
    }
    Some(event_from_u16(raw))
}

pub fn decode_event(value: u64) -> Option<KeyboardInput> {
    let value = value.checked_sub(1)?;
    let decoded = u32::try_from(value >> 16).ok()?;
    Some(KeyboardInput {
        raw: decode_raw(value as u16)?,
        decoded: decode_key(decoded),
    })
}
