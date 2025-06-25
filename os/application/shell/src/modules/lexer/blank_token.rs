use crate::modules::lexer::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind, TokenStatus};

pub struct BlankTokenContextFactory {}

impl TokenContextFactory for BlankTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Valid,
            is_pipe_open: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: prev_clx.in_quote,
            arg_kind: prev_clx.arg_kind.clone(),
            status: prev_clx.status.clone(),
            is_pipe_open: prev_clx.is_pipe_open,
        }
    }
}
