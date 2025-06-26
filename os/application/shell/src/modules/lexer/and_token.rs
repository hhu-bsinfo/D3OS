use crate::modules::lexer::{
    lexer::ParserError,
    token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind, TokenStatus},
};

pub struct AndTokenContextFactory {}

impl TokenContextFactory for AndTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        let error = ParserError::create(Some(
            "If you want to use a and condition, try moving && between commands (Example: cmd1 && cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No && condition')",
        ));

        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: TokenStatus::Error(error),
            is_pipe_open: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            status: prev_clx.status.clone(),
            is_pipe_open: false,
        }
    }
}
