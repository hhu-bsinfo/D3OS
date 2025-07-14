use alloc::string::{String, ToString};

use crate::{
    event::event_handler::Error,
    token::{
        definition::{
            ARGUMENT_TOKEN_DEFINITION, BACKGROUND_TOKEN_DEFINITION, BLANK_TOKEN_DEFINITION, COMMAND_TOKEN_DEFINITION,
            FILE_TOKEN_DEFINITION, LOGICAL_AND_TOKEN_DEFINITION, LOGICAL_OR_TOKEN_DEFINITION, PIPE_TOKEN_DEFINITION,
            QUOTE_END_TOKEN_DEFINITION, QUOTE_START_TOKEN_DEFINITION, REDIRECT_IN_APPEND_TOKEN_DEFINITION,
            REDIRECT_IN_TRUNCATE_TOKEN_DEFINITION, REDIRECT_OUT_APPEND_TOKEN_DEFINITION,
            REDIRECT_OUT_TRUNCATE_TOKEN_DEFINITION, SEPARATOR_TOKEN_DEFINITION, TokenDefinition,
        },
        token::{Token, TokenContext, TokenKind, TokenStatus},
    },
};

pub struct TokenFactory {}

impl TokenFactory {
    pub fn create_first(kind: TokenKind, content: String) -> Token {
        let definition = Self::get_definition(&kind);
        let dto = (definition.first_token_fn)(&content);

        let clx = TokenContext {
            pos: 0,
            line_pos: 0,
            cmd_pos_in_segment: dto.cmd_pos_in_segment,
            require_segment: dto.require_segment,
            require_file: dto.require_file,
            in_quote: dto.in_quote,
            is_end_of_line: dto.is_end_of_line,
        };

        let status = if dto.error_reason.is_some() {
            TokenStatus::Error(Error::new(
                "Invalid command line".to_string(),
                Some(dto.error_reason.unwrap().to_string()),
            ))
        } else if let Some(status) = Self::get_incomplete_status(&clx) {
            status
        } else {
            TokenStatus::Valid
        };

        Token::new(kind, content, clx, status)
    }

    pub fn create_next(prev: &Token, kind: TokenKind, content: String) -> Token {
        let definition = Self::get_definition(&kind);
        let dto = (definition.next_token_fn)(prev, &content);
        let prev_clx = prev.clx();

        let clx = TokenContext {
            pos: prev.clx().pos + 1,
            line_pos: prev.clx().line_pos + prev.len(),
            cmd_pos_in_segment: dto.cmd_pos_in_segment.unwrap_or(prev_clx.cmd_pos_in_segment.clone()),
            require_segment: dto.require_segment.unwrap_or(prev_clx.require_segment),
            require_file: dto.require_file.unwrap_or(prev_clx.require_file),
            in_quote: dto.in_quote.unwrap_or(prev_clx.in_quote),
            is_end_of_line: dto.is_end_of_line.unwrap_or(prev_clx.is_end_of_line),
        };

        let status = if prev.status().is_error() {
            prev.status().clone()
        } else if let Some(rule) = definition.error_rules.iter().find(|rule| (rule.condition)(prev)) {
            let error = Error::new("Invalid command line".to_string(), Some(rule.reason.to_string()));
            TokenStatus::Error(error)
        } else if let Some(status) = Self::get_incomplete_status(&clx) {
            status
        } else {
            TokenStatus::Valid
        };

        Token::new(kind, content, clx, status)
    }

    fn get_definition(kind: &TokenKind) -> &'static TokenDefinition {
        match *kind {
            TokenKind::Command => &COMMAND_TOKEN_DEFINITION,
            TokenKind::Argument => &ARGUMENT_TOKEN_DEFINITION,
            TokenKind::File => &FILE_TOKEN_DEFINITION,
            TokenKind::Blank => &BLANK_TOKEN_DEFINITION,
            TokenKind::QuoteStart => &QUOTE_START_TOKEN_DEFINITION,
            TokenKind::QuoteEnd => &QUOTE_END_TOKEN_DEFINITION,
            TokenKind::Pipe => &PIPE_TOKEN_DEFINITION,
            TokenKind::RedirectInTruncate => &REDIRECT_IN_TRUNCATE_TOKEN_DEFINITION,
            TokenKind::RedirectInAppend => &REDIRECT_IN_APPEND_TOKEN_DEFINITION,
            TokenKind::RedirectOutTruncate => &REDIRECT_OUT_TRUNCATE_TOKEN_DEFINITION,
            TokenKind::RedirectOutAppend => &REDIRECT_OUT_APPEND_TOKEN_DEFINITION,
            TokenKind::And => &LOGICAL_AND_TOKEN_DEFINITION,
            TokenKind::Or => &LOGICAL_OR_TOKEN_DEFINITION,
            TokenKind::Separator => &SEPARATOR_TOKEN_DEFINITION,
            TokenKind::Background => &BACKGROUND_TOKEN_DEFINITION,
        }
    }

    fn get_incomplete_status(clx: &TokenContext) -> Option<TokenStatus> {
        let reason = if clx.in_quote.is_some() {
            "Quote has not been closed"
        } else if clx.require_segment {
            "Expected command but got end of line"
        } else if clx.require_file {
            "Expected file but got end of line"
        } else {
            return None;
        };

        Some(TokenStatus::Incomplete(Error::new(
            "Invalid command line".to_string(),
            Some(reason.to_string()),
        )))
    }
}
