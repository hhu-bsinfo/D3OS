use crate::modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind};

pub struct FileTokenContextFactory {}

impl TokenContextFactory for FileTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: None,
            require_cmd: false,
            require_file: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: None,
            in_quote: prev_clx.in_quote,
            arg_kind: ArgumentKind::None,
            error: prev_clx.error,
            require_cmd: false,
            require_file: false,
        }
    }
}
