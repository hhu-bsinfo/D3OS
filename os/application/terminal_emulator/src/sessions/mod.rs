/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sessions                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Per-session screen multiplexer for the terminal emulator.       ║
   ║                                                                         ║
   ║ This module owns, per live session, the endpoints (`out` reader, `ctl`  ║
   ║ reader, lazily-opened `in` writer), a semantic terminal model, and a    ║
   ║ canonical line editor. Every loop it drains *every* session's `out`     ║
   ║ FIFO into that session's model (so background apps never block on the   ║
   ║ bounded pipe), and renders only the active session's damage to the      ║
   ║ framebuffer. Switching tabs presents the target session's semantic      ║
   ║ viewport in one repaint - no history is replayed.                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

pub(crate) mod screen;

use alloc::collections::btree_map::BTreeMap;
use alloc::rc::Rc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use concurrent::thread;
use log::error;
use log::{info, warn};
use naming::shared_types::OpenOptions;
use terminal_lib::session::{CtlRecord, Session, WaitState, read_ctl, spawn_supervisor};

use screen::SessionScreen;
use crate::terminal::lfb_terminal::LFBTerminal;
use crate::util::banner::create_banner_string;
use crate::worker::canonical::CanonicalAction;


/// Maximum number of concurrent sessions.
///
/// This cap is a drain-all / memory / supervisor cost bound:
/// every live session forces the emulator to drain its bounded `out` pipe each loop,
/// keeps a per-session screen model resident, and runs a per-session supervisor thread.
/// Ids are `u8` and never reused within a boot, so the snapshot represents a set with holes
/// (not slots 0..7); this number bounds concurrency, not the id space.
const MAX_SESSIONS: usize = 8;

/// Bytes drained from a session `out` pipe per loop iteration.
const OUT_READ_CHUNK: usize = 256;

pub struct SessionMux {
    terminal: Rc<LFBTerminal>,
    sessions: BTreeMap<u8, Arc<Session<SessionScreen>>>,
    active: u8,
    next_id: u8,
}

impl SessionMux {
    pub fn new(terminal: Rc<LFBTerminal>) -> Self {
        let mut s = Self {
            terminal,
            sessions: BTreeMap::new(),
            active: 0,
            next_id: 0,
        };
        s.new_session();
        s
    }

    pub(super) fn check_for_exited_shells(&mut self) {
        let exited: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_id, session)| session.exited())
            .map(|(id, _session)| id)
            .copied()
            .collect();
        for id in exited {
            self.close_session(id);
        }
    }

    /// Drain every session's output: feed it to that session's semantic model
    /// (so background writers never block on the bounded pipe) and repaint the
    /// active session's damaged rows.
    pub fn drain_outputs(&mut self) {
        let active = self.active;
        let terminal = self.terminal.clone();
        for (id, session) in self.sessions.iter() {
            let mut screen = session.screen.try_lock().expect("failed to get lock on session screen");
            let mut buffer = [0u8; OUT_READ_CHUNK];
            let Some(handle) = screen.out else {
                continue;
            };
            let read = naming::read(handle, &mut buffer).unwrap_or(0);
            if read == 0 {
                continue;
            }
            let damage = screen.feed(&buffer[0..read]);
            if *id == active {
                terminal.present_damage(screen.model(), damage);
            }
        }
    }

    /// The active session's foreground control record, if a foreground app has
    /// published one.
    pub fn active_ctl(&self) -> Option<CtlRecord> {
        let session = self.sessions.get(&self.active)?;
        let handle = session.screen.try_lock().expect("failed to get lock on session screen").ctl?;
        match read_ctl(handle) {
            Ok(record) if record.is_published() => Some(record),
            _ => None,
        }
    }

    /// Open the active session's `in` writer once its foreground app is waiting,
    /// so the app's blocking reader `open()` can complete.
    pub fn ensure_active_in_writer(&mut self) {
        let waiting = self
            .active_ctl()
            .is_some_and(|record| record.wait == WaitState::Waiting);
        if !waiting {
            return;
        }
        if let Some(session) = self.sessions.get_mut(&self.active) {
            let mut screen = session.screen.try_lock().expect("failed to get lock on session screen");
            if screen.in_writer.is_none() {
                screen.in_writer = naming::open(
                    &session.in_path(), OpenOptions::WRITEONLY,
                ).ok();
            }
        }
    }

    /// Deliver a decoded input buffer to the active session's foreground app.
    pub fn deliver(&mut self, bytes: &[u8]) {
        self.ensure_active_in_writer();
        if let Some(session) = self.sessions.get(&self.active) {
            if let Some(handle) = session.screen.try_lock().expect("failed to get lock on session screen").in_writer {
                let _ = naming::write(handle, bytes);
            }
        }
    }

    /// Apply a canonical-mode edit to the active session: echo into that
    /// session's semantic terminal (repainting it) and, on submit, deliver the
    /// completed line to its foreground app.
    pub fn handle_canonical(&mut self, action: CanonicalAction) {
        let active = self.active;
        let terminal = self.terminal.clone();

        let submit = {
            let Some(session) = self.sessions.get_mut(&active) else {
                return;
            };
            let mut screen = session.screen.try_lock().expect("failed to get lock on session screen");
            let (damage, submit) = screen.edit(action);
            if !damage.is_clean() {
                terminal.present_damage(screen.model(), damage);
            }
            submit
        };

        if let Some(bytes) = submit {
            self.deliver(&bytes);
        }
    }

    /// Repaint the active session's full viewport (used after GUI mode returns).
    pub fn present_active(&self) {
        if let Some(session) = self.sessions.get(&self.active) {
            self.terminal.present_full(session.screen.try_lock().expect("failed to get lock on session screen").model());
        }
    }

    pub(super) fn new_session(&mut self) {
        // TODO: move parts of this into SessionEntry::new
        info!("creating a new session");
        if self.sessions.len() >= MAX_SESSIONS {
            warn!("can't create any more sessions");
            return; // Refuse gracefully; see MAX_SESSIONS.
        }
        let id = self.next_id;
        self.next_id += 1;
        
        let session = Arc::new(Session::with_id(
            id as usize,
            SessionScreen::new(self.terminal.screen_size()),
        ));
        session.create();
        {
            let mut screen = session.screen.try_lock().expect("failed to get lock on screen");
            screen.out = naming::open(
                &session.out_path(),
                OpenOptions::READONLY | OpenOptions::NONBLOCK,
            ).ok();
            if screen.out.is_none() {
                error!("failed to open stdout for new session");
            }
            screen.ctl = naming::open(&session.ctl_path(), OpenOptions::READWRITE).ok();
            if screen.ctl.is_none() {
                error!("failed to open ctl for new session");
            }
            // Seed the banner so each tab shows it (survives tab switches via the
            // semantic model, not a byte replay ring).
            let _ = screen.feed(create_banner_string().as_bytes());
        }
        spawn_supervisor(session.clone());
        self.sessions.insert(id, session);

        self.active = id;
        self.publish_tabs();
        self.present_active();
    }

    pub(super) fn close_active(&mut self) {
        self.close_session(self.active);
    }

    /// Close a session.
    /// 
    /// This may be an active or an inactive one.
    /// If we're closing the last one, spawn a new one.
    fn close_session(&mut self, id: u8) {
        if let Some(session) = self.sessions.remove(&id) {
            // Terminate the closed session's shell so its supervisor's join() returns
            // and the supervisor stops (the session is no longer alive).
            let shell_tid = session.shell_tid();
            if shell_tid != 0 {
                thread::kill(shell_tid);
            }
            
            let screen = session.screen.try_lock().expect("failed to get lock on session screen");
            if let Some(handle) = screen.out {
                let _ = naming::close(handle);
            }
            if let Some(handle) = screen.ctl {
                let _ = naming::close(handle);
            }
            if let Some(handle) = screen.in_writer {
                let _ = naming::close(handle);
            }
            // TODO: delete the pipes and the folder
        }

        // We need to always have at least one session.
        if self.sessions.len() == 0 {
            self.new_session();
        }
    
        // Don't keep pointing at a session that is gone: input and repaints
        // would silently go nowhere. Choose the lowest remaining session.
        if self.active == id {
            self.active = *self.sessions.keys().next().expect("no sessions");
        }

        self.publish_tabs();
        self.present_active();
    }

    pub(crate) fn switch_relative(&mut self, direction: i32) {
        let live: Vec<u8> = self.sessions.keys().copied().collect();
        if live.len() <= 1 {
            return;
        }
    
        let position = live.iter().position(
            |&id| id == self.active
        ).unwrap_or_default() as i32;
        let len = live.len() as i32;
        let next = (((position + direction) % len) + len) % len; // TODO
        self.active = live[next as usize];
        
        self.publish_tabs();
        self.present_active();
    }

    /// Push the current live session ids and active id into the terminal so the
    /// status bar can render tabs. The `update_tabs` lock is released before any
    /// drawing call, so this must not run while the `display` lock is held.
    fn publish_tabs(&self) {
        let ids: alloc::vec::Vec<u8> = self.sessions.keys().copied().collect();
        self.terminal.update_tabs(&ids, self.active);
    }
}
