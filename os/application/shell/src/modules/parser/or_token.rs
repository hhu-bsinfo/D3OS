use alloc::string::ToString;
use spin::Lazy;

use crate::{
    event::event_handler::Error,
    modules::parser::token::{TokenContext, TokenContextFactory, TokenKind},
};

static LOGICAL_OR_BEFORE_CMD_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some(
            "If you want to use a or condition, try moving || between commands (Example: cmd1 || cmd2)\nIf you want || as normal char, try wrapping it in parentheses (Example: echo 'No || condition')".to_string(),
        ),
    )
});

static LOGICAL_OR_INSTEAD_OF_FILE_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Expected a filename but got ||".to_string()),
    )
});

static LOGICAL_OR_AFTER_BACKGROUND_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Expected end of line".to_string()),
    )
});

pub struct OrTokenContextFactory {}

impl TokenContextFactory for OrTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos: None,
            in_quote: None,
            error: Some(&LOGICAL_OR_BEFORE_CMD_ERROR),
            require_cmd: true,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, prev_content: &str, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&LOGICAL_OR_BEFORE_CMD_ERROR)
            } else if prev_clx.require_file {
                Some(&LOGICAL_OR_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&LOGICAL_OR_AFTER_BACKGROUND_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_content.len(),
            cmd_pos: None,
            in_quote: None,
            error,
            require_cmd: true,
            require_file: false,
            has_background: prev_clx.has_background,
        }
    }
}
