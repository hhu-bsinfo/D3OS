use crate::modules::lexer::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind, TokenStatus};

pub struct QuoteStartTokenContextFactory {}

impl TokenContextFactory for QuoteStartTokenContextFactory {
    fn create_first(_kind: &TokenKind, ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: Some(ch),
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Incomplete,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: Some(ch),
            arg_kind: prev_clx.arg_kind.clone(),
            status: prev_clx.status.clone(),
        }
    }
}
