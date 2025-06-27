use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const PIPE_BEFORE_CMD_ERROR: Error = Error::new(
    "Invalid command line",
    Some(
        "If you want to use a pipe, try moving | between commands (Example: cmd1 | cmd2)\nIf you want | as normal char, try wrapping it in parentheses (Example: echo 'No | pipe')",
    ),
);

pub struct PipeTokenContextFactory {}

impl TokenContextFactory for PipeTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: Some(&PIPE_BEFORE_CMD_ERROR),
            require_cmd: true,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&PIPE_BEFORE_CMD_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error,
            require_cmd: true,
        }
    }
}
