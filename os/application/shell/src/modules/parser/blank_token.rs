use crate::modules::parser::token::{TokenContext, TokenContextFactory, TokenKind};

pub struct BlankTokenContextFactory {}

impl TokenContextFactory for BlankTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos: None,
            in_quote: None,
            error: None,
            require_cmd: false,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, prev_content: &str, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_content.len(),
            cmd_pos: prev_clx.cmd_pos,
            in_quote: prev_clx.in_quote,
            error: prev_clx.error,
            require_cmd: prev_clx.require_cmd,
            require_file: prev_clx.require_file,
            has_background: prev_clx.has_background,
        }
    }
}
