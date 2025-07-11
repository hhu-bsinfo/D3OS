use crate::token::token::{Token, TokenContext, TokenContextFactory};

pub struct QuoteEndTokenContextFactory {}

impl TokenContextFactory for QuoteEndTokenContextFactory {
    fn create_first(_content: &str) -> TokenContext {
        panic!("The first token can not be a end of quote");
    }

    fn create_after(prev_token: &Token, _content: &str) -> TokenContext {
        let prev_clx = prev_token.clx();
        TokenContext {
            pos: prev_clx.pos + 1,
            line_pos: prev_clx.line_pos + prev_token.len(),
            cmd_pos: prev_clx.cmd_pos,
            in_quote: None,
            error: prev_clx.error,
            require_cmd: prev_clx.require_cmd,
            require_file: prev_clx.require_file,
            has_background: prev_clx.has_background,
        }
    }
}
