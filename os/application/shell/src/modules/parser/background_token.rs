use alloc::string::ToString;
use spin::Lazy;

use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

static BG_BEFORE_CMD_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some(
            "If you want to use a background execution, try moving & after the command (Example: cmd1 arg1 arg2 &)
If you want & as normal char, try wrapping it in parentheses (Example: echo 'No & background execution')"
                .to_string(),
        ),
    )
});

static BACKGROUND_INSTEAD_OF_FILE_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Expected a filename but got &".to_string()),
    )
});

static MULTIPLE_BACKGROUND_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Expected end of line".to_string()),
    )
});

pub struct BackgroundTokenContextFactory {}

impl TokenContextFactory for BackgroundTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: Some(&BG_BEFORE_CMD_ERROR),
            require_cmd: false,
            require_file: false,
            has_background: true,
        }
    }

    fn create_after(prev_clx: &TokenContext, prev_content: &str, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.cmd_pos.is_none() {
                Some(&BG_BEFORE_CMD_ERROR)
            } else if prev_clx.require_file {
                Some(&BACKGROUND_INSTEAD_OF_FILE_ERROR)
            } else if prev_clx.has_background {
                Some(&MULTIPLE_BACKGROUND_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_content.len(),
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error,
            require_cmd: false,
            require_file: false,
            has_background: true,
        }
    }
}
