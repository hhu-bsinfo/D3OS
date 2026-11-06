/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: read                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Read input from the terminal.                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Authors: Fabian Ruhland, 31.8.2024, HHU,                                ║
   ║          Aymen Sellami                                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::{String, ToString};
use naming::shared_types::OpenOptions;
use pc_keyboard::{DecodedKey, KeyEvent};
use spin::Mutex;
use stream::event_from_u16;
use crate::session::{write_ctl, CtlRecord, Session, WaitState};
use crate::{DecodedKeyType, TerminalMode};

/// Per-process input endpoints, opened lazily and kept open for the process
/// lifetime so the session pipe is never reset between reads.
struct InputEndpoints {
    /// Control record handle (mode/wait publishing).
    ctl: Option<usize>,
    /// Blocking stdin reader, used by canonical line reads.
    in_blocking: Option<usize>,
    /// Non-blocking stdin reader, used by fluid/raw key polling.
    in_nonblock: Option<usize>,
}

static INPUT: Mutex<InputEndpoints> = Mutex::new(InputEndpoints {
    ctl: None,
    in_blocking: None,
    in_nonblock: None,
});

/// Publish the requested mode and wait-state on the control plane.
fn publish(mode: TerminalMode, wait: WaitState) {
    let mut input = INPUT.lock();
    if input.ctl.is_none() {
        input.ctl = naming::open(&Session::current().ctl_path(), OpenOptions::READWRITE).ok();
    }
    if let Some(handle) = input.ctl {
        let _ = write_ctl(handle, &CtlRecord::new(mode, wait));
    }
}

/// Obtain the cached stdin reader handle for the requested blocking mode,
/// opening it on first use. Returns `None` if the session pipe cannot be opened.
fn in_handle(nonblock: bool) -> Option<usize> {
    let mut input = INPUT.lock();
    let slot = if nonblock { &mut input.in_nonblock } else { &mut input.in_blocking };
    if slot.is_none() {
        let session = Session::current();
        let mut flags = OpenOptions::READONLY;
        if nonblock {
            flags |= OpenOptions::NONBLOCK;
        }
        *slot = naming::open(&session.in_path(), flags).ok();
    }
    *slot
}

/// Read a single 2-byte key frame from a non-blocking stdin handle.
///
/// Returns `None` when no input is currently buffered. The emulator always
/// writes keys as 2-byte frames, but on a multi-core system a poll can observe
/// a write mid-flight and read only the first byte; in that case we complete
/// the frame so the stream stays 2-byte aligned.
fn read_key(handle: usize) -> Option<[u8; 2]> {
    let mut buffer = [0u8; 2];
    let mut got = naming::read(handle, &mut buffer).unwrap_or(0);
    if got == 0 {
        return None;
    }
    while got < buffer.len() {
        got += naming::read(handle, &mut buffer[got..]).unwrap_or(0);
    }
    Some(buffer)
}

/// Read from terminal in canonical mode.
///
/// The terminal will echo.
/// The application will block until 'Enter' is pressed.
/// Command line editing is enabled.
/// Returns written line.
/// TODO: Silently handles errors by returning an empty string, find out if this is fine?
pub fn read() -> String {
    let mut buffer: [u8; 128] = [0; 128];

    // Publish before opening/reading so the emulator opens its writer end and
    // performs canonical line editing for this reader.
    publish(TerminalMode::Canonical, WaitState::Waiting);

    let read_bytes = match in_handle(false) {
        Some(handle) => naming::read(handle, &mut buffer).unwrap_or(0),
        None => 0,
    };

    publish(TerminalMode::Canonical, WaitState::Idle);

    String::from_utf8_lossy(&buffer[0..read_bytes]).to_string()
}

/// Read from terminal in fluid mode.
///
/// The terminal will not echo.
/// The application will not block.
/// Returns decoded key as well as raw special keys.
pub fn read_fluid() -> Option<DecodedKey> {
    publish(TerminalMode::Fluid, WaitState::Waiting);

    let handle = in_handle(true)?;
    let buffer = read_key(handle)?;

    let key_type = DecodedKeyType::from(buffer[0]);
    let key = buffer[1];

    match key_type {
        DecodedKeyType::Unicode => Some(DecodedKey::Unicode(key as char)),
        DecodedKeyType::RawKey => Some(DecodedKey::RawKey(unsafe { core::mem::transmute(key) })),
    }
}

/// Read from terminal in raw mode.
///
/// The terminal will not echo.
/// The application will not block.
/// Returns raw undecoded key.
pub fn read_raw() -> Option<KeyEvent> {
    publish(TerminalMode::Raw, WaitState::Waiting);

    let handle = in_handle(true)?;
    let buffer = read_key(handle)?;

    let raw = u16::from_ne_bytes(buffer);
    Some(event_from_u16(raw))
}
