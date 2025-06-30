use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const REDIRECT_OUT_APPEND_BEFORE_CMD_ERROR: Error = Error::new(
    "Invalid command line",
    Some(
        "If you want to redirect some output, try moving >> after a command (Example: cmd1 >> file)\nIf you want >> as normal char, try wrapping it in parentheses (Example: echo 'No >> redirection')",
    ),
);

const REDIRECT_OUT_APPEND_INSTEAD_OF_FILE_ERROR: Error =
    Error::new("Invalid command line", Some("Expected a filename but got >>"));

const REDIRECT_OUT_APPEND_AFTER_BACKGROUND_ERROR: Error =
    Error::new("Invalid command line", Some("Expected end of line"));

pub struct RedirectOutAppendTokenContextFactory {}

impl TokenContextFactory for RedirectOutAppendTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: Some(&REDIRECT_OUT_APPEND_BEFORE_CMD_ERROR),
            require_cmd: false,
            require_file: true,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&REDIRECT_OUT_APPEND_BEFORE_CMD_ERROR)
            } else if prev_clx.require_file {
                Some(&REDIRECT_OUT_APPEND_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&REDIRECT_OUT_APPEND_AFTER_BACKGROUND_ERROR)
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
            require_cmd: false,
            require_file: true,
            has_background: prev_clx.has_background,
        }
    }
}
