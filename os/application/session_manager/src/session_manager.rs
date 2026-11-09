/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: session_manager                                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Standalone userspace owner of terminal sessions/tabs.           ║
   ║                                                                         ║
   ║ The session manager owns session-id allocation, the `/term/<id>/`       ║
   ║ namespace lifecycle, active-session selection, and the policy for       ║
   ║ launching shells/apps into sessions. In terminal-boot mode the kernel   ║
   ║ boots this process; it launches the `terminal_emulator` (renderer) and  ║
   ║ a shell per session, tagging each shell with the `D3OS_TERM_SESSION`    ║
   ║ env descriptor so the shell and its children resolve the right session. ║
   ║                                                                         ║
   ║ It coordinates with the emulator over `/term/manager/{ctl,cmd}`:        ║
   ║   - it publishes the authoritative live-session snapshot (active id +   ║
   ║     generation + 256-bit live mask) on the `ctl` record,                ║
   ║   - it consumes UI commands (new/close/next/prev tab) on `cmd`.         ║
   ║                                                                         ║
   ║ A per-session supervisor thread restarts a session's shell when it      ║
   ║ exits, and stops once the session is closed.                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicU8, Ordering};

use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::vec::Vec;

use concurrent::thread;
use naming::shared_types::OpenOptions;
use spin::Mutex;
use terminal::manager::{self, cmd, ManagerCtl};
use terminal::session::{Session, SESSION_DESCRIPTOR_KEY};

#[allow(unused_imports)]
use runtime::*;

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
    next_id: AtomicU8::new(1),
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

/// Publish the authoritative live-session snapshot on the manager `ctl` record:
/// the active id, live count, a freshly bumped generation, and the live mask.
fn publish_ctl(ctl: usize) {
    let (count, generation, mask) = {
        let sessions = STATE.sessions.lock();
        let generation = STATE.generation.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let count = sessions.len() as u8;
        let mask = manager::live_mask_from_ids(sessions.keys().copied());
        (count, generation, mask)
    };
    manager::write_ctl(ctl, &ManagerCtl::new(
        STATE.active.load(Ordering::SeqCst), count, generation, mask,
    )).expect("failed to write manager ctl");
}

/// Restart loop for a single session's shell.
///
/// Runs in its own thread so the manager's command loop never blocks on
/// `join()`. Restarts the shell when it exits normally (e.g. the user runs the
/// `exit` built-in) and terminates once the session is closed.
fn supervise(id: u8) {
    loop {
        if !is_alive(id) {
            break;
        }

        let descriptor = format!("{}={}", SESSION_DESCRIPTOR_KEY, id);
        let env: Vec<&str> = alloc::vec![descriptor.as_str()];

        match thread::start_application_with_env("shell", Vec::new(), env) {
            Some(shell) => {
                set_shell_tid(id, shell.id());
                let _ = shell.join();
            }
            None => {
                thread::sleep(500);
            }
        }

        // If the session was closed while the shell ran, stop; otherwise the
        // shell exited on its own and we restart it.
        if !is_alive(id) {
            break;
        }
    }
}

fn spawn_supervisor(id: u8) {
    thread::create(move || supervise(id));
}

fn handle_command(command: u8, ctl: usize) {
    match command {
        cmd::NEW_TAB => new_tab(ctl),
        cmd::CLOSE_ACTIVE => close_active(ctl),
        cmd::NEXT_TAB => switch_relative(ctl, 1),
        cmd::PREV_TAB => switch_relative(ctl, -1),
        _ => {}
    }
}

fn new_tab(ctl: usize) {
    let id = {
        let mut sessions = STATE.sessions.lock();
        let live = sessions.len();
        if live >= MAX_SESSIONS {
            return; // Refuse gracefully; see MAX_SESSIONS.
        }
        let id = STATE.next_id.fetch_add(1, Ordering::SeqCst);
        sessions.insert(id, SessionEntry { shell_tid: 0 });
        STATE.active.store(id, Ordering::SeqCst);
        id
    };

    let _ = Session::with_id(id as usize).create();
    spawn_supervisor(id);
    publish_ctl(ctl);
}

fn close_active(ctl: usize) {
    let shell_tid = {
        let mut sessions = STATE.sessions.lock();
        // Always keep at least one session alive.
        if sessions.len() <= 1 {
            return;
        }

        let id = STATE.active.load(Ordering::SeqCst);
        let shell_tid = sessions.get(&id).map(|entry| entry.shell_tid).unwrap_or(0);
        sessions.remove(&id);

        // Pick the lowest remaining session id as the new active session.
        if let Some(&next) = sessions.keys().next() {
            STATE.active.store(next, Ordering::SeqCst);
        }
        shell_tid
    };

    // Terminate the closed session's shell so its supervisor's join() returns
    // and the supervisor stops (the session is no longer alive).
    if shell_tid != 0 {
        thread::kill(shell_tid);
    }
    publish_ctl(ctl);
}

fn switch_relative(ctl: usize, direction: i32) {
    {
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
    }
    publish_ctl(ctl);
}

#[unsafe(no_mangle)]
pub fn main() {
    manager::create_namespace();

    let ctl = naming::open(&manager::ctl_path(), OpenOptions::READWRITE)
        .expect("failed to open manager ctl");

    // Create the initial session 0 and record it.
    Session::with_id(0).create();
    STATE.sessions.lock().insert(0, SessionEntry { shell_tid: 0 });
    publish_ctl(ctl);
    spawn_supervisor(0);

    // Launch the renderer. It connects back to the manager namespace and opens
    // its endpoints for each session it finds in the `ctl` snapshot.
    thread::start_application("terminal_emulator", Vec::new())
        .expect("failed to start terminal_emulator");

    // Open the command FIFO reader. the emulator's `cmd` writer open blocks
    // until this reader is present.
    // It is non-blocking so the command loop doesn't stall when no key has been pressed.
    let cmd_r = naming::open(&manager::cmd_path(), OpenOptions::READONLY | OpenOptions::NONBLOCK)
        .expect("failed to open manager cmd");
    // Command loop: react to UI commands from the emulator.
    loop {
        let mut byte = [0u8; 1];
        if let Ok(1) = naming::read(cmd_r, &mut byte) {
            handle_command(byte[0], ctl);
        }
        thread::sleep(20);
    }
}
