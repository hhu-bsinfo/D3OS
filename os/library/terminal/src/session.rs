/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: session                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Userspace terminal session model built on `naming`.             ║
   ║                                                                         ║
   ║ A terminal session is a directory namespace `/term/<id>/` holding three ║
   ║ named objects:                                                          ║
   ║   - `in`  : FIFO carrying application stdin  (emulator -> app)          ║
   ║   - `out` : FIFO carrying application stdout (app -> emulator)          ║
   ║   - `ctl` : a regular file holding a fixed-layout control record        ║
   ║                                                                         ║
   ║ stdin/stdout stay plain byte streams. All terminal-mode and input-wait  ║
   ║ metadata lives on the `ctl` record, never inside the data streams.      ║
   ║                                                                         ║
   ║ Discovery is centralized here: `Session::current()` is the single       ║
   ║ resolver every application goes through. It reads the launch-time       ║
   ║ `D3OS_TERM_SESSION` descriptor from the process environment and falls   ║
   ║ back to the default session when no descriptor is present.              ║
   ║                                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

#[cfg(feature = "manager")]
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alloc::format;
use alloc::string::{String, ToString};

#[cfg(feature = "manager")]
use alloc::sync::Arc;
use naming::shared_types::SeekOrigin;
use spin::Mutex;
use syscall::return_vals::Errno;

use crate::TerminalMode;

/// Root of the terminal session namespace.
pub const TERM_ROOT: &str = "/term";

/// Fallback session id used by processes that are not launched with an explicit
/// session descriptor, such as the session manager and terminal emulator.
pub const DEFAULT_SESSION_ID: usize = 0;

/// Environment key carrying the launch-time terminal session descriptor.
///
/// The descriptor is the session id as a decimal string, never a raw open
/// handle: handles are indices into the kernel's global open-object table and
/// have no per-process meaning, so they cannot be inherited. A child re-opens
/// its endpoints by path from the id.
pub const SESSION_DESCRIPTOR_KEY: &str = "D3OS_TERM_SESSION";

/// Size of the fixed-layout control record stored in `ctl`. The record never
/// grows; it is always read/written in full at offset 0.
pub const CTL_RECORD_SIZE: usize = 32;

/// Current control-record layout version. A `version` of 0 means "no record
/// has been published yet" (a freshly `touch`ed, empty file decodes to this).
pub const CTL_VERSION: u8 = 1;

// Byte offsets within the control record.
const OFF_VERSION: usize = 0;
const OFF_MODE: usize = 1;
const OFF_WAIT: usize = 2;
// byte 3 reserved (flags)
const OFF_ROWS: usize = 4; // u16, little endian (reserved for resize)
const OFF_COLS: usize = 6; // u16, little endian (reserved for resize)
// bytes 8..24 reserved for a future foreground-process id (u128)
// bytes 24..32 reserved

/// Input-wait flag carried on the control plane.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WaitState {
    /// The foreground app is not currently waiting for input.
    Idle,
    /// The foreground app is blocked waiting for input in the recorded mode.
    Waiting,
}

impl WaitState {
    fn to_byte(self) -> u8 {
        match self {
            WaitState::Idle => 0,
            WaitState::Waiting => 1,
        }
    }

    #[cfg(feature = "manager")]
    fn from_byte(b: u8) -> Self {
        match b {
            1 => WaitState::Waiting,
            _ => WaitState::Idle,
        }
    }
}

/// The pollable control record exchanged on the `ctl` plane.
#[derive(Debug, Clone, Copy)]
pub struct CtlRecord {
    version: u8,
    pub mode: TerminalMode,
    pub wait: WaitState,
    pub rows: u16,
    pub cols: u16,
}

impl CtlRecord {
    /// A zeroed/unpublished record. `is_published()` is `false` for this value.
    pub fn empty() -> Self {
        Self {
            version: 0,
            mode: TerminalMode::Canonical,
            wait: WaitState::Idle,
            rows: 0,
            cols: 0,
        }
    }

    /// A published record describing the app's requested mode and wait-state.
    pub fn new(mode: TerminalMode, wait: WaitState) -> Self {
        Self {
            version: CTL_VERSION,
            mode,
            wait,
            rows: 0,
            cols: 0,
        }
    }

    /// `true` once a process has actually written a record.
    pub fn is_published(&self) -> bool {
        self.version != 0
    }

    fn encode(&self) -> [u8; CTL_RECORD_SIZE] {
        let mut buf = [0u8; CTL_RECORD_SIZE];
        buf[OFF_VERSION] = self.version;
        buf[OFF_MODE] = usize::from(self.mode) as u8;
        buf[OFF_WAIT] = self.wait.to_byte();
        buf[OFF_ROWS..OFF_ROWS + 2].copy_from_slice(&self.rows.to_le_bytes());
        buf[OFF_COLS..OFF_COLS + 2].copy_from_slice(&self.cols.to_le_bytes());
        buf
    }

    #[cfg(feature = "manager")]
    fn decode(buf: &[u8]) -> Self {
        if buf.len() < CTL_RECORD_SIZE || buf[OFF_VERSION] == 0 {
            return Self::empty();
        }
        let mode = TerminalMode::from(buf[OFF_MODE] as usize);
        let wait = WaitState::from_byte(buf[OFF_WAIT]);
        let rows = u16::from_le_bytes([buf[OFF_ROWS], buf[OFF_ROWS + 1]]);
        let cols = u16::from_le_bytes([buf[OFF_COLS], buf[OFF_COLS + 1]]);
        Self {
            version: buf[OFF_VERSION],
            mode,
            wait,
            rows,
            cols,
        }
    }
}

/// A resolved terminal session: just the absolute paths of its objects. All
/// path knowledge is confined to this type.
/// 
/// For the manager, this type also contains metadata.
pub struct Session<S> {
    base: String,

    /// Thread id of the session's current shell (`0` if not yet running). Used
    /// to terminate the shell when the tab is closed.
    #[cfg(feature = "manager")]
    pub(super) shell_tid: AtomicUsize,
    // The screen is only ever accessed by the main thread, but Rust requires this.
    // We still need to store the S somewhere, even if it's ().
    // TODO: try to replace this with PhantomData
    pub screen: Mutex<S>,
    #[cfg(feature = "manager")]
    exited: AtomicBool,
}

impl Session<()> {
    /// Resolve the session the current process belongs to.
    ///
    /// Resolves the launch-time descriptor [`SESSION_DESCRIPTOR_KEY`] from the
    /// process environment (set by the session manager when launching an app
    /// into a specific session, and inherited automatically by children). When
    /// no descriptor is present - e.g. the terminal emulator itself,
    /// launched directly by the kernel - this falls back to the
    /// well-known default session [`DEFAULT_SESSION_ID`].
    pub fn current() -> Self {
        match bootstrap_env::var(SESSION_DESCRIPTOR_KEY) {
            Some(descriptor) => Self::from_descriptor(&descriptor, ()),
            None => Self::with_id(DEFAULT_SESSION_ID, ()),
        }
    }
}

impl<S> Session<S> {
    /// Resolve a session by explicit id.
    pub fn with_id(id: usize, screen: S) -> Self {
        Session {
            base: format!("{}/{}", TERM_ROOT, id),
            screen: Mutex::new(screen),
            #[cfg(feature = "manager")]
            shell_tid: AtomicUsize::default(),
            #[cfg(feature = "manager")]
            exited: AtomicBool::default(),
        }
    }

    /// Resolve a session from a launch-time descriptor string.
    ///
    /// The descriptor is the folder carried in [`SESSION_DESCRIPTOR_KEY`].
    pub fn from_descriptor(descriptor: &str, screen: S) -> Self {
        Self {
            base: descriptor.trim().to_string(),
            screen: Mutex::new(screen),
            #[cfg(feature = "manager")]
            shell_tid: AtomicUsize::default(),
            #[cfg(feature = "manager")]
            exited: AtomicBool::default(),
        }
    }

    pub fn in_path(&self) -> String {
        format!("{}/in", self.base)
    }

    pub fn out_path(&self) -> String {
        format!("{}/out", self.base)
    }

    pub fn ctl_path(&self) -> String {
        format!("{}/ctl", self.base)
    }

    /// Create the session namespace and its three objects.
    ///
    /// Best-effort and intended to be called by the session owner
    /// (`session_manager`). Errors on already-existing components are ignored
    /// so a restart does not fail. The `ctl` file is left empty, which decodes
    /// to an unpublished record.
    #[cfg(feature = "manager")]
    pub fn create(&self) {
        let _ = naming::mkdir(TERM_ROOT);
        let _ = naming::mkdir(&self.base);
        let _ = naming::mkfifo(&self.in_path());
        let _ = naming::mkfifo(&self.out_path());
        let _ = naming::touch(&self.ctl_path());
    }

    /// Loop for a single session's shell
    ///
    /// It runs in its own thread, so the manager's command loop never blocks on
    /// `join()`. When the shell exits, it terminates the session.
    #[cfg(feature = "manager")]
    fn supervise(self: Arc<Self>) {
        use alloc::vec::Vec;
        use core::sync::atomic::Ordering;
        use log::{info, error};
        
        info!("starting shell for session {}", self.base);
    
        let descriptor = format!("{}={}", SESSION_DESCRIPTOR_KEY, self.base);
        let env: Vec<&str> = alloc::vec![descriptor.as_str()];
    
        if let Some(shell) = concurrent::thread::start_application_with_env("shell", Vec::new(), env) {

            self.shell_tid.store(shell.id(), Ordering::SeqCst);
            match shell.join() {
                Ok(_) => info!("shell for session {} exited", self.base),
                Err(e) => error!("shell for session {} failed to start: {:?}", self.base, e),
            }
        } else {
            error!("failed to start shell for session {}", self.base);
        }

        // Update the state, so that the session will be removed.
        self.exited.store(true, Ordering::SeqCst);
    }

    #[cfg(feature = "manager")]
    pub fn exited(&self) -> bool {
        self.exited.load(Ordering::SeqCst)
    }

    #[cfg(feature = "manager")]
    pub fn shell_tid(&self) -> usize {
        self.shell_tid.load(Ordering::SeqCst)
    }
}

/// Read the full control record from an open `ctl` handle (offset is reset to
/// 0 first, so the same handle can be polled repeatedly).
#[cfg(feature = "manager")]
pub fn read_ctl(handle: usize) -> Result<CtlRecord, Errno> {
    naming::seek(handle, 0, SeekOrigin::Start)?;
    let mut buf = [0u8; CTL_RECORD_SIZE];
    let n = naming::read(handle, &mut buf)?;
    // Incomplete control record is treated as unpublished.
    if n < CTL_RECORD_SIZE {
        return Ok(CtlRecord::empty());
    }
    Ok(CtlRecord::decode(&buf))
}

/// Write the full control record to an open `ctl` handle at offset 0.
pub(super) fn write_ctl(handle: usize, record: &CtlRecord) -> Result<(), Errno> {
    naming::seek(handle, 0, SeekOrigin::Start)?;
    let buf = record.encode();
    naming::write(handle, &buf)?;
    Ok(())
}

#[cfg(feature = "manager")]
pub fn spawn_supervisor<S: Send + 'static>(session: Arc<Session<S>>) {
    concurrent::thread::create(move || session.supervise());
}
