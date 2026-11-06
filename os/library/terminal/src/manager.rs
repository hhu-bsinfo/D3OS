/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: manager                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Coordination protocol between the standalone `session_manager`  ║
   ║         and the `terminal_emulator`, built on `naming`.                 ║
   ║                                                                         ║
   ║ The manager owns a small namespace `/term/manager/` holding two         ║
   ║ objects:                                                                ║
   ║   - `ctl`   : a fixed-layout tmpfs control record (pollable state):     ║
   ║               the active session id, generation, and a 256-bit live     ║
   ║               session mask (the authoritative membership snapshot).     ║
   ║   - `cmd`   : a FIFO carrying one-shot UI commands emulator -> manager  ║
   ║               (new/close/next/prev tab).                                ║
   ║                                                                         ║
   ║ Membership is current state, not a stream: the full live set travels    ║
   ║ on the `ctl` record so the emulator can poll it repeatedly and diff it  ║
   ║ against its local map without consuming anything. Only one-shot UI      ║
   ║ commands (the user pressed a tab key) travel on the `cmd` FIFO so they  ║
   ║ are consumed exactly once.                                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use naming::shared_types::SeekOrigin;
use syscall::return_vals::Errno;

use crate::session::TERM_ROOT;

/// Root of the manager coordination namespace.
pub const MANAGER_ROOT: &str = "/term/manager";

/// One-shot UI commands sent emulator -> manager on the `cmd` FIFO, one byte
/// per command.
pub mod cmd {
    /// Create a new session/tab and make it active.
    pub const NEW_TAB: u8 = 1;
    /// Close the currently active session/tab.
    pub const CLOSE_ACTIVE: u8 = 2;
    /// Switch to the next live session/tab.
    pub const NEXT_TAB: u8 = 3;
    /// Switch to the previous live session/tab.
    pub const PREV_TAB: u8 = 4;
}

/// Number of bytes in the live-session mask: one bit per possible `u8` session
/// id, so the whole never-reuse id space (256 ids) is representable (even if it has holes).
pub const LIVE_MASK_BYTES: usize = 32;

/// Size of the fixed-layout manager control record. Always read/written in full
/// at offset 0, like the per-session `ctl` record.
pub const MANAGER_CTL_SIZE: usize = 40;

/// Current manager control-record version. `0` means "not published yet".
pub const MANAGER_CTL_VERSION: u8 = 2;

const OFF_VERSION: usize = 0;
const OFF_ACTIVE: usize = 1; // active session id (u8)
const OFF_COUNT: usize = 2; // live session count (u8)
const OFF_GENERATION: usize = 3; // bumped on every change (u8)
const OFF_MASK: usize = 4; // bytes 4-35: 256-bit live session mask (LIVE_MASK_BYTES bytes)
// bytes 36..40 reserved

/// The pollable manager control record: the authoritative membership snapshot.
///
/// `live_mask` is the source of truth for which sessions exist. Bit `id` is byte
/// `id / 8`, bit `id % 8` (LSB-first); a set bit means id `id` is live. Because
/// the bit index *is* the id, the snapshot represents a set with holes and the
/// protocol maximum is the full `u8` id space.
#[derive(Debug, Clone, Copy)]
pub struct ManagerCtl {
    version: u8,
    pub active_id: u8,
    pub session_count: u8,
    pub generation: u8,
    live_mask: [u8; LIVE_MASK_BYTES],
}

impl ManagerCtl {
    /// An unpublished record. `is_published()` is `false` for this value.
    pub fn empty() -> Self {
        Self {
            version: 0,
            active_id: 0,
            session_count: 0,
            generation: 0,
            live_mask: [0u8; LIVE_MASK_BYTES],
        }
    }

    /// A published record describing the current active session, counters, and
    /// live-session mask.
    pub fn new(active_id: u8, session_count: u8, generation: u8, live_mask: [u8; LIVE_MASK_BYTES]) -> Self {
        Self {
            version: MANAGER_CTL_VERSION,
            active_id,
            session_count,
            generation,
            live_mask,
        }
    }

    /// `true` once the manager has actually written a record.
    pub fn is_published(&self) -> bool {
        self.version != 0
    }

    /// `true` if session `id` is set in the live mask.
    pub fn is_live(&self, id: u8) -> bool {
        (self.live_mask[(id / 8) as usize] >> (id % 8)) & 1 != 0
    }

    /// The live session ids in ascending order, decoded from the mask.
    pub fn live_ids(&self) -> Vec<u8> {
        let mut ids = Vec::new();
        for id in 0u8..=255 {
            if self.is_live(id) {
                ids.push(id);
            }
        }
        ids
    }

    fn encode(&self) -> [u8; MANAGER_CTL_SIZE] {
        let mut buf = [0u8; MANAGER_CTL_SIZE];
        buf[OFF_VERSION] = self.version;
        buf[OFF_ACTIVE] = self.active_id;
        buf[OFF_COUNT] = self.session_count;
        buf[OFF_GENERATION] = self.generation;
        buf[OFF_MASK..OFF_MASK + LIVE_MASK_BYTES].copy_from_slice(&self.live_mask);
        buf
    }

    fn decode(buf: &[u8]) -> Self {
        if buf.len() < MANAGER_CTL_SIZE || buf[OFF_VERSION] == 0 {
            return Self::empty();
        }
        let mut live_mask = [0u8; LIVE_MASK_BYTES];
        live_mask.copy_from_slice(&buf[OFF_MASK..OFF_MASK + LIVE_MASK_BYTES]);
        Self {
            version: buf[OFF_VERSION],
            active_id: buf[OFF_ACTIVE],
            session_count: buf[OFF_COUNT],
            generation: buf[OFF_GENERATION],
            live_mask,
        }
    }
}

/// Build a live-session mask from an iterator of live ids (bit `id` set).
pub fn live_mask_from_ids<I: IntoIterator<Item = u8>>(ids: I) -> [u8; LIVE_MASK_BYTES] {
    let mut mask = [0u8; LIVE_MASK_BYTES];
    for id in ids {
        mask[(id / 8) as usize] |= 1 << (id % 8);
    }
    mask
}

pub fn ctl_path() -> String {
    format!("{}/ctl", MANAGER_ROOT)
}

pub fn cmd_path() -> String {
    format!("{}/cmd", MANAGER_ROOT)
}

/// Create the manager namespace and its objects.
///
/// Best-effort and intended to be called once by the manager at startup. Errors
/// on already-existing components are ignored so a restart does not fail.
pub fn create_namespace() {
    let _ = naming::mkdir(TERM_ROOT);
    let _ = naming::mkdir(MANAGER_ROOT);
    let _ = naming::touch(&ctl_path());
    let _ = naming::mkfifo(&cmd_path());
}

/// Read the full manager control record from an open `ctl` handle (offset is
/// reset to 0 first, so the same handle can be polled repeatedly).
pub fn read_ctl(handle: usize) -> Result<ManagerCtl, Errno> {
    naming::seek(handle, 0, SeekOrigin::Start)?;
    let mut buf = [0u8; MANAGER_CTL_SIZE];
    let n = naming::read(handle, &mut buf)?;
    if n < MANAGER_CTL_SIZE {
        return Ok(ManagerCtl::empty());
    }
    Ok(ManagerCtl::decode(&buf))
}

/// Write the full manager control record to an open `ctl` handle at offset 0.
pub fn write_ctl(handle: usize, record: &ManagerCtl) -> Result<(), Errno> {
    naming::seek(handle, 0, SeekOrigin::Start)?;
    let buf = record.encode();
    naming::write(handle, &buf)?;
    Ok(())
}
