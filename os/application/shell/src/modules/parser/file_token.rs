use alloc::string::ToString;
use spin::Lazy;

use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

pub struct FileTokenContextFactory {}

static FILE_AFTER_BACKGROUND_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Expected end of line".to_string()),
    )
});

impl TokenContextFactory for FileTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: None,
            arg_kind: ArgumentKind::None,
            error: None,
            require_cmd: false,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, prev_content: &str, _kind: &TokenKind, _ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.has_background {
                Some(&FILE_AFTER_BACKGROUND_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_content.len(),
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: None,
            in_quote: prev_clx.in_quote,
            arg_kind: ArgumentKind::None,
            error,
            require_cmd: false,
            require_file: false,
            has_background: prev_clx.has_background,
        }
    }
}
