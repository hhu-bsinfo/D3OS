use crate::{
    event::event_handler::Error,
    modules::parser::token::{ArgumentKind, TokenContext, TokenContextFactory, TokenKind},
};

const NESTED_QUOTE_ERROR: Error = Error::new("Invalid command line", Some("Nesting quotes is not supported"));

pub struct QuoteStartTokenContextFactory {}

impl TokenContextFactory for QuoteStartTokenContextFactory {
    fn create_first(_kind: &TokenKind, ch: char) -> TokenContext {
        TokenContext {
            pos: 0,
            cmd_pos: None,
            short_flag_pos: None,
            in_quote: Some(ch),
            arg_kind: ArgumentKind::None,
            error: None,
            require_cmd: false,
        }
    }

    fn create_after(prev_clx: &TokenContext, _kind: &TokenKind, ch: char) -> TokenContext {
        let error = prev_clx.error.or_else(|| {
            if prev_clx.in_quote.is_some() {
                Some(&NESTED_QUOTE_ERROR)
            } else {
                None
            }
        });

        TokenContext {
            pos: prev_clx.pos + 1,
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: Some(ch),
            arg_kind: prev_clx.arg_kind.clone(),
            error,
            require_cmd: prev_clx.require_cmd,
        }
    }
}
