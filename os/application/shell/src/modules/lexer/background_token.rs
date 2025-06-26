use crate::modules::lexer::{
    lexer::ParserError,
    token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind, TokenStatus},
};

pub struct BackgroundTokenContextFactory {}

impl TokenContextFactory for BackgroundTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        let error = ParserError::create(Some(
            "If you want to use a background execution, try moving & after the command (Example: cmd1 arg1 arg2 &)\nIf you want & as normal char, try wrapping it in parentheses (Example: echo 'No & background execution')",
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
