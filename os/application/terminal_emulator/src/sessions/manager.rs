/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sessions::manager                                               ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ The session manager owns session-id allocation, the `/term/<id>/`       ║
   ║ namespace lifecycle, active-session selection, and the policy for       ║
   ║ launching shells/apps into sessions. It launches a shell per session,   ║
   ║ tagging them with the `D3OS_TERM_SESSION` env descriptor, so the shell  ║              
   ║ and its children resolve the right session.                             ║
   ║                                                                         ║
   ║ A per-session supervisor thread monitors a session's shell and stops    ║
   ║ once the session is closed.                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::sync::atomic::{AtomicU8, Ordering};

use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::vec::Vec;

use concurrent::thread;
use log::{error, info};
use spin::{Mutex, MutexGuard};
use terminal::session::{Session, SESSION_DESCRIPTOR_KEY};

/// Maximum number of concurrent sessions.
///
/// This cap is a drain-all / memory / supervisor cost bound:
/// every live session forces the emulator to drain its bounded `out` pipe each loop,
/// keeps a per-session screen model resident, and runs a per-session supervisor thread.
/// Ids are `u8` and never reused within a boot, so the snapshot represents a set with holes
/// (not slots 0..7); this number bounds concurrency, not the id space.
const MAX_SESSIONS: usize = 8;

/// Per-session bookkeeping kept by the manager.
struct SessionEntry {
    /// Thread id of the session's current shell (`0` if not yet running). Used
    /// to terminate the shell when the tab is closed.
    shell_tid: usize,
}

struct ManagerState {
    sessions: Mutex<BTreeMap<u8, SessionEntry>>,
    active: AtomicU8,
    next_id: AtomicU8,
    generation: AtomicU8,
}

static STATE: ManagerState = ManagerState {
    sessions: Mutex::new(BTreeMap::new()),
    active: AtomicU8::new(0),
    next_id: AtomicU8::new(0),
    generation: AtomicU8::new(0),
};

fn is_alive(id: u8) -> bool {
    STATE.sessions.lock().contains_key(&id)
}

fn set_shell_tid(id: u8, tid: usize) {
    STATE
        .sessions
        .lock()
        .get_mut(&id)
        .expect("session is mising")
        .shell_tid = tid;
}

/// Publish the authoritative live-session snapshot.
fn publish() {
    STATE.generation.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
}

/// Loop for a single session's shell
///
/// It runs in its own thread, so the manager's command loop never blocks on
/// `join()`. When the shell exits, it terminates the session.
fn supervise(id: u8) {
    // Don't start if we shouldn't be alive.
    if !is_alive(id) {
        return;
    }
    info!("starting shell for session {id}");

    let descriptor = format!("{}={}", SESSION_DESCRIPTOR_KEY, id);
    let env: Vec<&str> = alloc::vec![descriptor.as_str()];

    if let Some(shell) = thread::start_application_with_env("shell", Vec::new(), env) {    
        set_shell_tid(id, shell.id());
        match shell.join() {
            Ok(_) => info!("shell for session {id} exited"),
            Err(e) => error!("shell for session {id} failed to start: {e:?}"),
        }
    } else {
        error!("failed to start shell for session {id}");
    }

    // If the shell is dead, but the session is still alive, close it.
    if is_alive(id) {
        close(id);
    }
}

fn spawn_supervisor(id: u8) {
    thread::create(move || supervise(id));
}

/// Create a new tab.
pub(crate) fn new_tab() {
    info!("creating a new tab");
    let mut sessions = STATE.sessions.lock();
    let live = sessions.len();
    if live >= MAX_SESSIONS {
        return; // Refuse gracefully; see MAX_SESSIONS.
    }
    let id = new_session(&mut sessions);
    STATE.active.store(id, Ordering::SeqCst);
    publish();
}

/// Create a new session and return its id.
/// 
/// This also spawns the shell.
fn new_session(sessions: &mut MutexGuard<'_, BTreeMap<u8, SessionEntry>>) -> u8 {
    let id = STATE.next_id.fetch_add(1, Ordering::SeqCst);
    sessions.insert(id, SessionEntry { shell_tid: 0 });
    let _ = Session::with_id(id.into()).create();
    spawn_supervisor(id);
    id
}

/// Close the currently active tab.
pub(crate) fn close_active() {
    close(STATE.active.load(Ordering::SeqCst));
}

/// Close a session.
/// 
/// This may be an active or an inactive one.
/// If we're closing the last one, spawn a new one.
fn close(id: u8) {
    let mut sessions = STATE.sessions.lock();

    let shell_tid = sessions
        .get(&id)
        .map(|entry| entry.shell_tid)
        .expect("session is missing");
    sessions.remove(&id);
    
    // We need to always have at least one session.
    if sessions.len() == 0 {
        new_session(&mut sessions);
    }

    // if we're closing the currently active session,
    // we need to pick a new one (the lowest remaining session)
    if id == STATE.active.load(Ordering::SeqCst) {
        STATE.active.store(
            *sessions.keys().next().expect("no sessions"), Ordering::SeqCst,
        );
    }

    // Terminate the closed session's shell so its supervisor's join() returns
    // and the supervisor stops (the session is no longer alive).
    if shell_tid != 0 {
        thread::kill(shell_tid);
    }
    publish();
}

pub(crate) fn next_tab() {
    switch_relative(1);
}

pub(crate) fn prev_tab() {
    switch_relative(-1);
}

fn switch_relative(direction: i32) {
    let sessions = STATE.sessions.lock();
    let live: Vec<u8> = sessions
        .iter()
        .map(|(id, _)| *id)
        .collect();
    if live.len() <= 1 {
        return;
    }

    let position = live.iter().position(
        |&id| id == STATE.active.load(Ordering::SeqCst)
    ).unwrap_or_default() as i32;
    let len = live.len() as i32;
    let next = (((position + direction) % len) + len) % len; // TODO
    STATE.active.store(live[next as usize], Ordering::SeqCst);
    publish();
}

pub(crate) fn init() {
    // Create the initial session 0 and record it.
    let mut sessions = STATE.sessions.lock();
    assert!(sessions.is_empty());
    new_session(&mut sessions);
    publish();
}

/// Get the current generation.
pub(crate) fn generation() -> u8 {
    STATE.generation.load(Ordering::SeqCst)
}

/// The pollable manager control record: the authoritative membership snapshot.
// TODO: do we actually need this?
#[derive(Debug, Clone)]
pub struct ManagerCtl {
    pub active_id: u8,
    pub session_count: u8,
    pub generation: u8,
    pub live_ids: Vec<u8>,
}

impl ManagerCtl {
    /// Get a current snapshot. Blocks.
    pub(super) fn get() -> Self {
        let sessions = STATE.sessions.lock();
        Self {
            active_id: STATE.active.load(Ordering::SeqCst),
            session_count: sessions.len().try_into().unwrap(),
            generation: STATE.generation.load(Ordering::SeqCst),
            live_ids: sessions.keys().copied().collect(),
        }
    }
}
