/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: ansi                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Session-local ANSI parser + semantic screen model.              ║
   ║                                                                         ║
   ║ `SemanticTerminal` pairs one `anstyle_parse::Parser` with one           ║
   ║ `TerminalModel`. Feeding bytes advances the parser, which drives the    ║
   ║ model's `Perform` backend. Because both live behind this owning type,   ║
   ║ each session keeps its own parser state (mid-escape sequences, SGR      ║
   ║ colors, cursor) and can be parsed independently of the framebuffer.     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use anstyle_parse::{Parser, Utf8Parser};

use super::model::{Damage, ScreenSize, TerminalModel};

pub struct SemanticTerminal {
    parser: Parser<Utf8Parser>,
    model: TerminalModel,
}

impl SemanticTerminal {
    pub fn new(size: ScreenSize) -> Self {
        Self {
            parser: Parser::<Utf8Parser>::new(),
            model: TerminalModel::new(size),
        }
    }

    /// Advance the parser/model over `bytes` and return the damage produced by
    /// this call only (the damage summary is reset at the start of each feed).
    pub fn feed(&mut self, bytes: &[u8]) -> Damage {
        self.model.reset_damage();
        for &byte in bytes {
            self.parser.advance(&mut self.model, byte);
        }
        self.model.damage
    }

    pub fn model(&self) -> &TerminalModel {
        &self.model
    }
}
