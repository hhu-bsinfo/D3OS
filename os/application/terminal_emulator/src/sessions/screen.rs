/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: screen                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: One session's screen state, independent of any I/O.             ║
   ║                                                                         ║
   ║ A `SessionScreen` owns everything that makes a tab a tab: its semantic  ║
   ║ terminal (parser, grid, cursor, colors, scrollback) and its canonical   ║
   ║ line editor. Feeding output and applying edits are pure functions of    ║
   ║ that state, which is what lets a tab switch repaint instead of replay.  ║
   ║ The FIFO handles live here too, but only `SessionMux` touches them.     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::vec::Vec;

use crate::terminal::ansi::SemanticTerminal;
use crate::terminal::model::{Damage, ScreenSize, TerminalModel};
use crate::worker::canonical::{CanonicalAction, CanonicalEditor};

pub struct SessionScreen {
    /// Non-blocking reader on the session `out` FIFO (app stdout).
    pub(crate) out: Option<usize>,
    /// Reader on the session `ctl` record (foreground mode/wait state).
    pub(crate) ctl: Option<usize>,
    /// Lazily-opened writer on the session `in` FIFO (app stdin).
    pub(crate) in_writer: Option<usize>,
    /// Semantic screen state: parser + grid + cursor + colors + scrollback.
    terminal: SemanticTerminal,
    /// Canonical-mode line editor for this session.
    canonical: CanonicalEditor,
}

impl SessionScreen {
    pub fn new(size: ScreenSize) -> Self {
        Self {
            out: None,
            ctl: None,
            in_writer: None,
            terminal: SemanticTerminal::new(size),
            canonical: CanonicalEditor::new(),
        }
    }

    /// Parse application output into this session's screen state.
    pub fn feed(&mut self, bytes: &[u8]) -> Damage {
        self.terminal.feed(bytes)
    }

    /// Apply a canonical-mode edit. The echo is rendered into this session's
    /// own screen rather than the framebuffer, so an unsubmitted line survives
    /// a switch to another tab and back. The second half of the result is the
    /// completed line, present only once the user submits it.
    pub fn edit(&mut self, action: CanonicalAction) -> (Damage, Option<Vec<u8>>) {
        let effect = self.canonical.apply(action);
        let damage = match effect.echo.is_empty() {
            true => Damage::none(),
            false => self.terminal.feed(effect.echo.as_bytes()),
        };
        (damage, effect.submit)
    }

    pub fn model(&self) -> &TerminalModel {
        self.terminal.model()
    }
}
