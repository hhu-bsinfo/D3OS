use syscall::{SystemCall, return_vals::Errno, syscall};

use crate::{DecodedKey, KeyEvent, KeyboardInput, Layout, wire};

/// Reads up to one event and its layout mapping from the shared D3OS keyboard.
/// With blocking enabled, waits for an event, including modifier releases.
/// This consumes the event from the system queue. Use the terminal's input API
/// for applications receiving input through a terminal session.
pub fn read_event(blocking: bool) -> Result<Option<KeyboardInput>, Errno> {
    let value = syscall(SystemCall::KeyboardRead, &[wire::READ_EVENT, usize::from(blocking)])?;
    match value {
        0 => Ok(None),
        _ => wire::decode_event(value as u64).map(Some).ok_or(Errno::EINVAL),
    }
}

/// Reads a raw event from the shared D3OS keyboard.
pub fn read_raw(blocking: bool) -> Result<Option<KeyEvent>, Errno> {
    Ok(read_event(blocking)?.map(|input| input.raw))
}

/// Reads one event from the shared D3OS keyboard and returns its decoded key.
/// A modifier release can produce `None` even with blocking enabled.
pub fn read_key(blocking: bool) -> Result<Option<DecodedKey>, Errno> {
    Ok(read_event(blocking)?.and_then(|input| input.decoded))
}

/// Gets the active system keyboard layout. Shortcut presses are applied as the
/// system keyboard stream is read.
pub fn get_layout() -> Result<Layout, Errno> {
    let id = syscall(SystemCall::KeyboardRead, &[wire::GET_LAYOUT, 0])?;
    Layout::from_id(id).ok_or(Errno::EINVAL)
}

/// Sets the active system keyboard layout for subsequent events.
pub fn set_layout(layout: Layout) -> Result<(), Errno> {
    syscall(SystemCall::KeyboardRead, &[wire::SET_LAYOUT, layout as usize]).map(|_| ())
}
