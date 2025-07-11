use alloc::string::ToString;
use spin::Lazy;

use crate::{
    event::event_handler::Error,
    token::token::{Token, TokenContext, TokenContextFactory},
};

static NESTED_QUOTE_ERROR: Lazy<Error> = Lazy::new(|| {
    Error::new(
        "Invalid command line".to_string(),
        Some("Nesting quotes is not supported".to_string()),
    )
});

pub struct QuoteStartTokenContextFactory {}

impl TokenContextFactory for QuoteStartTokenContextFactory {
    fn create_first(content: &str) -> TokenContext {
        TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos: None,
            in_quote: Some(content.chars().next().expect("Expect at least one char")),
            error: None,
            require_cmd: false,
            require_file: false,
            has_background: false,
        }
    }

    fn create_after(prev_token: &Token, content: &str) -> TokenContext {
        let prev_clx = prev_token.clx();
        let error = prev_clx.error.or_else(|| {
            if prev_clx.in_quote.is_some() {
                Some(&NESTED_QUOTE_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_token.len(),
            cmd_pos: prev_clx.cmd_pos,
            in_quote: Some(content.chars().next().expect("Expect at least one char")),
            error,
            require_cmd: prev_clx.require_cmd,
            require_file: prev_clx.require_file,
            has_background: prev_clx.has_background,
        }
    }
}
