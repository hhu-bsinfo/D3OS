use crate::{
    event::event_handler::Error,
    modules::lexer::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const BG_BEFORE_CMD_ERROR: Error = Error::new(
    "Invalid command line",
    Some(
        "If you want to use a background execution, try moving & after the command (Example: cmd1 arg1 arg2 &)\nIf you want & as normal char, try wrapping it in parentheses (Example: echo 'No & background execution')",
    ),
);

pub struct BackgroundTokenContextFactory {}

impl TokenContextFactory for BackgroundTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: Some(&BG_BEFORE_CMD_ERROR),
            require_cmd: true,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&BG_BEFORE_CMD_ERROR)
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
