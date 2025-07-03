use crate::modules::parser::token::{TokenContext, TokenContextFactory, TokenKind};

pub struct QuoteEndTokenContextFactory {}

impl TokenContextFactory for QuoteEndTokenContextFactory {
    fn create_first(_kind: &TokenKind, _ch: char) -> TokenContext {
        panic!("The first token can not be a end of quote");
    }

    fn create_after(prev_clx: &TokenContext, prev_content: &str, _kind: &TokenKind, _ch: char) -> TokenContext {
        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_content.len(),
            cmd_pos: prev_clx.cmd_pos,
            short_flag_pos: prev_clx.short_flag_pos,
            in_quote: None,
            arg_kind: prev_clx.arg_kind.clone(),
            error: prev_clx.error,
            require_cmd: prev_clx.require_cmd,
            require_file: prev_clx.require_file,
            has_background: prev_clx.has_background,
        }
    }
}
