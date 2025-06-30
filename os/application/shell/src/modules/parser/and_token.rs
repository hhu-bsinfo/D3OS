use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const LOGICAL_AND_BEFORE_CMD_ERROR: Error = Error::new(
    "Invalid command line",
    Some(
        "If you want to use a and condition, try moving && between commands (Example: cmd1 && cmd2)\nIf you want && as normal char, try wrapping it in parentheses (Example: echo 'No && condition')",
    ),
);

const LOGICAL_AND_INSTEAD_OF_FILE_ERROR: Error =
    Error::new("Invalid command line", Some("Expected a filename but got &&"));

const LOGICAL_AND_AFTER_BACKGROUND_ERROR: Error = Error::new("Invalid command line", Some("Expected end of line"));

pub struct AndTokenContextFactory {}

impl TokenContextFactory for AndTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: Some(&LOGICAL_AND_BEFORE_CMD_ERROR),
            require_cmd: true,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&LOGICAL_AND_BEFORE_CMD_ERROR)
            } else if prev_clx.require_file {
                Some(&LOGICAL_AND_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&LOGICAL_AND_AFTER_BACKGROUND_ERROR)
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
            require_file: false,
            has_background: prev_clx.has_background,
        }
    }
}
