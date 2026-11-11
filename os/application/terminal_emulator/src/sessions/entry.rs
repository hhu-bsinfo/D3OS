//! Session entry
//! 
//! This modules contains the management part of a session.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alloc::sync::Arc;
use alloc::format;
use alloc::vec::Vec;

use concurrent::thread;
use log::{error, info};
use spin::mutex::Mutex;
use terminal::session::SESSION_DESCRIPTOR_KEY;

use super::screen::SessionScreen;

/// Per-session bookkeeping kept by the manager.
pub(super) struct SessionEntry {
    id: u8,
    /// Thread id of the session's current shell (`0` if not yet running). Used
    /// to terminate the shell when the tab is closed.
    pub(super) shell_tid: AtomicUsize,
    // The screen is only ever accessed by the main thread, but Rust requires this.
    pub(super) screen: Mutex<SessionScreen>,
    pub(super) exited: AtomicBool,
}

impl SessionEntry {
    pub(super) fn new(id: u8, screen: SessionScreen) -> Arc<Self> {
        Arc::new(Self {
            id,
            shell_tid: AtomicUsize::new(0),
            screen: Mutex::new(screen),
            exited: AtomicBool::new(false),
        })
    }

    /// Loop for a single session's shell
    ///
    /// It runs in its own thread, so the manager's command loop never blocks on
    /// `join()`. When the shell exits, it terminates the session.
    fn supervise(self: Arc<Self>) {
        info!("starting shell for session {}", self.id);
    
        let descriptor = format!("{}={}", SESSION_DESCRIPTOR_KEY, self.id);
        let env: Vec<&str> = alloc::vec![descriptor.as_str()];
    
        if let Some(shell) = thread::start_application_with_env("shell", Vec::new(), env) {
            self.shell_tid.store(shell.id(), Ordering::SeqCst);
            match shell.join() {
                Ok(_) => info!("shell for session {} exited", self.id),
                Err(e) => error!("shell for session {} failed to start: {:?}", self.id, e),
            }
        } else {
            error!("failed to start shell for session {}", self.id);
        }

        // Update the state, so that the session will be removed.
        self.exited.store(true, Ordering::SeqCst);
    }
}

pub(super) fn spawn_supervisor(session: Arc<SessionEntry>) {
    thread::create(move || session.supervise());
}
