use crate::modules::lexer::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind, TokenStatus};

pub struct CommandTokenContextFactory {}

impl TokenContextFactory for CommandTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: Some(0),
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Valid,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: Some(prev_clx.pos + 1),
            short_flag_pos: None,
            in_quote: prev_clx.in_quote,
            arg_kind: ArgumentKind::None,
            status: prev_clx.status.clone(),
        }
    }
}
