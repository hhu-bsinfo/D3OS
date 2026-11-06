/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sessions                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Per-session screen multiplexer for the terminal emulator.       ║
   ║                                                                         ║
   ║ The emulator is a pure renderer/input client of the standalone          ║
   ║ `session_manager`. This module owns, per live session, the endpoints    ║
   ║ (`out` reader, `ctl` reader, lazily-opened `in` writer), a semantic      ║
   ║ terminal model, and a canonical line editor. Every loop it drains        ║
   ║ *every* session's `out` FIFO into that session's model (so background     ║
   ║ apps never block on the bounded pipe), and renders only the active        ║
   ║ session's damage to the framebuffer. Switching tabs presents the target   ║
   ║ session's semantic viewport in one repaint - no history is replayed.     ║
   ║                                                                         ║
   ║ It also speaks the manager protocol: each loop it polls the manager      ║
   ║ `ctl` record (an authoritative live-session snapshot: active id +        ║
   ║ generation + 256-bit live mask), diffs that set against its local map    ║
   ║ to open/close endpoints, applies the active id, and forwards UI          ║
   ║ commands (new/close/next/prev tab) on the `cmd` FIFO.                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

pub(crate) mod screen;

use alloc::collections::btree_map::BTreeMap;
use alloc::rc::Rc;

use naming::shared_types::OpenOptions;
use terminal_lib::manager;
use terminal_lib::session::{read_ctl, CtlRecord, Session, WaitState};

use crate::terminal::lfb_terminal::LFBTerminal;
use crate::util::banner::create_banner_string;
use crate::worker::canonical::CanonicalAction;

use screen::SessionScreen;

/// Bytes drained from a session `out` pipe per loop iteration.
const OUT_READ_CHUNK: usize = 256;

/// Sentinel "no active session selected yet" so the first manager `ctl` poll
/// always triggers an initial render.
const NO_ACTIVE: u8 = u8::MAX;

pub struct SessionMux {
    terminal: Rc<LFBTerminal>,
    /// Manager `ctl` record reader (pollable live-session snapshot).
    manager_ctl: Option<usize>,
    /// Manager `cmd` FIFO writer (UI commands emulator -> manager).
    manager_cmd: Option<usize>,
    sessions: BTreeMap<u8, SessionScreen>,
    active: u8,
    /// Generation of the last snapshot applied, so unchanged snapshots are
    /// skipped. `None` until the first published snapshot is seen.
    last_generation: Option<u8>,
}

impl SessionMux {
    pub fn new(terminal: Rc<LFBTerminal>) -> Self {
        Self {
            terminal,
            manager_ctl: None,
            manager_cmd: None,
            sessions: BTreeMap::new(),
            active: NO_ACTIVE,
            last_generation: None,
        }
    }

    /// Connect to the manager namespace.
    ///
    /// The only rendezvous is the `cmd` FIFO: opening it WRITEONLY blocks until
    /// the manager's `cmd` reader is present. The `ctl` record is a pollable
    /// tmpfs file with no rendezvous, so it is opened afterwards.
    pub fn connect(&mut self) {
        self.manager_cmd = naming::open(&manager::cmd_path(), OpenOptions::WRITEONLY).ok();
        self.manager_ctl = naming::open(&manager::ctl_path(), OpenOptions::READWRITE).ok();
    }

    /// Poll the manager `ctl` snapshot: skip if the generation is unchanged,
    /// otherwise diff the live-session set against the local map (add/remove
    /// endpoints) and then apply the active id.
    pub fn poll_snapshot(&mut self) {
        let Some(handle) = self.manager_ctl else {
            return;
        };
        let Ok(record) = manager::read_ctl(handle) else {
            return;
        };
        if !record.is_published() {
            return;
        }
        if self.last_generation == Some(record.generation) {
            return;
        }

        // Diff membership before touching the active id, so a newly announced id
        // has its endpoints open before it can be selected.
        for id in record.live_ids() {
            if !self.sessions.contains_key(&id) {
                self.add_session(id);
            }
        }
        let stale: alloc::vec::Vec<u8> = self
            .sessions
            .keys()
            .copied()
            .filter(|id| !record.is_live(*id))
            .collect();
        for id in stale {
            self.remove_session(id);
        }

        if record.active_id != self.active && self.sessions.contains_key(&record.active_id) {
            self.switch_to(record.active_id);
        }

        self.last_generation = Some(record.generation);
    }

    /// Drain every session's output: feed it to that session's semantic model
    /// (so background writers never block on the bounded pipe) and repaint the
    /// active session's damaged rows.
    pub fn drain_outputs(&mut self) {
        let active = self.active;
        let terminal = self.terminal.clone();
        for (id, screen) in self.sessions.iter_mut() {
            let Some(handle) = screen.out else {
                continue;
            };
            let mut buffer = [0u8; OUT_READ_CHUNK];
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
        let screen = self.sessions.get(&self.active)?;
        let handle = screen.ctl?;
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
        let active = self.active;
        if let Some(screen) = self.sessions.get_mut(&active) {
            if screen.in_writer.is_none() {
                let session = Session::with_id(active as usize);
                screen.in_writer = naming::open(&session.in_path(), OpenOptions::WRITEONLY).ok();
            }
        }
    }

    /// Deliver a decoded input buffer to the active session's foreground app.
    pub fn deliver(&mut self, bytes: &[u8]) {
        self.ensure_active_in_writer();
        if let Some(screen) = self.sessions.get(&self.active) {
            if let Some(handle) = screen.in_writer {
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
            let Some(screen) = self.sessions.get_mut(&active) else {
                return;
            };
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

    /// Forward a one-shot UI command to the manager.
    pub fn send_command(&mut self, command: u8) {
        if let Some(handle) = self.manager_cmd {
            let _ = naming::write(handle, &[command]);
        }
    }

    /// Repaint the active session's full viewport (used after GUI mode returns).
    pub fn present_active(&self) {
        if let Some(screen) = self.sessions.get(&self.active) {
            self.terminal.present_full(screen.model());
        }
    }

    fn add_session(&mut self, id: u8) {
        if self.sessions.contains_key(&id) {
            return;
        }
        let mut screen = SessionScreen::new(self.terminal.screen_size());
        let session = Session::with_id(id as usize);
        screen.out = naming::open(
            &session.out_path(),
            OpenOptions::READONLY | OpenOptions::NONBLOCK,
        )
        .ok();
        screen.ctl = naming::open(&session.ctl_path(), OpenOptions::READWRITE).ok();
        // Seed the banner so each tab shows it (survives tab switches via the
        // semantic model, not a byte replay ring).
        let _ = screen.feed(create_banner_string().as_bytes());
        self.sessions.insert(id, screen);

        self.publish_tabs();

        if self.active == id {
            self.present_active();
        }
    }

    fn remove_session(&mut self, id: u8) {
        if let Some(screen) = self.sessions.remove(&id) {
            if let Some(handle) = screen.out {
                let _ = naming::close(handle);
            }
            if let Some(handle) = screen.ctl {
                let _ = naming::close(handle);
            }
            if let Some(handle) = screen.in_writer {
                let _ = naming::close(handle);
            }
        }

        // Don't keep pointing at a session that is gone: input and repaints
        // would silently go nowhere until the manager published a new active
        // id. Dropping to the sentinel makes the next `ctl` poll re-select.
        if self.active == id {
            self.active = NO_ACTIVE;
        }

        self.publish_tabs();
    }

    fn switch_to(&mut self, id: u8) {
        self.active = id;
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
