use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

use super::{parser::Parser, token::Token};

#[derive(Debug)]
pub struct LexicalParser {
    tokens: Vec<Token>,
}

impl LexicalParser {
    pub const fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    fn add_token_from_string(&mut self, string: &str) {
        if string.is_empty() {
            return;
        }

        match string {
            // Check for unambiguous tokens (pipes, redirects, ...):
            " " => self.handle_add_whitespace(),
            // Check for ambiguous tokens (commands & arguments):
            _ => self.handle_add_ambiguous(string.to_string()),
        }
    }

    fn add_token_from_char(&mut self, ch: char) {
        match ch {
            // Check for unambiguous tokens (whitespaces, pipes, redirects, ...):
            ' ' => self.handle_add_whitespace(),

            // Check for ambiguous tokens (commands & arguments):
            _ => self.handle_add_ambiguous(String::from(ch)),
        }
    }

    fn handle_add_whitespace(&mut self) {
        match self.tokens.last() {
            Some(token) => {
                if matches!(token, Token::Whitespace) {
                    return;
                }

                self.tokens.push(Token::Whitespace);
            }
            None => self.tokens.push(Token::Whitespace),
        }
    }

    fn handle_add_ambiguous(&mut self, str: String) {
        // If no token exists, then added token is command
        if self.tokens.is_empty() {
            self.tokens.push(Token::Command(str));
            return;
        }

        let last_token = self
            .tokens
            .last()
            .expect("Expected Parser to have at least one token");

        // If last token is neigther command or argument, then added token is command
        if !matches!(last_token, Token::Command(_) | Token::Argument(_)) {
            self.tokens.push(Token::Command(str));
            return;
        }

        // Else, added token must be an argument
        self.tokens.push(Token::Argument(str));
    }

    fn push_to_last_token(&mut self, ch: char) {
        let mut update_token = self
            .tokens
            .last()
            .expect("Expected Parser to have at least one token")
            .to_string();

        update_token.push(ch);
        self.update_last_token(&update_token);
    }

    fn pop_from_last_token(&mut self) {
        let mut update_token = self
            .tokens
            .last()
            .expect("Expected Parser to have at least one token")
            .to_string();

        update_token.pop();
        self.update_last_token(&update_token);
    }

    fn update_last_token(&mut self, string: &str) {
        self.tokens.pop();

        // If the updated string is empty, then the old token will just be deleted
        if string.is_empty() {
            return;
        }

        self.add_token_from_string(string);
    }
}

impl Parser for LexicalParser {
    fn push(&mut self, ch: char) {
        if self.tokens.is_empty() || ch == ' ' {
            self.add_token_from_char(ch);
            info!("{:?}", self.tokens);
            return;
        }

        self.push_to_last_token(ch);
        info!("{:?}", self.tokens);
    }

    fn pop(&mut self) {
        if self.tokens.is_empty() {
            info!("{:?}", self.tokens);
            return;
        }

        self.pop_from_last_token();
        info!("{:?}", self.tokens);
    }

    fn parse(&mut self) {
        info!("Parsing: {:?}", self.tokens);
    }

    fn reset(&mut self) {
        self.tokens.clear();
    }
}

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec};

    use super::*;

    //////////////////////////////////////////////////
    // General
    //////////////////////////////////////////////////

    #[test]
    fn test_empty_input_returns_empty_vec() {
        let parser = LexicalParser::new();
        assert_eq!(parser.tokens, vec![]);
    }

    //////////////////////////////////////////////////
    // push
    //////////////////////////////////////////////////

    #[test]
    fn test_push_single_word_input_returns_command_token() {
        let mut parser = LexicalParser::new();
        parser.push('e');
        parser.push('x');
        parser.push('i');
        parser.push('t');
        assert_eq!(parser.tokens, vec![Token::Command(String::from("exit"))]);
    }

    #[test]
    fn test_push_multi_word_input_returns_command_first_and_else_arg_tokens() {
        let mut parser = LexicalParser::new();
        parser.push('c');
        parser.push(' ');
        parser.push('a');
        parser.push(' ');
        parser.push('a');
        assert_eq!(
            parser.tokens,
            vec![
                Token::Command("c".to_string()),
                Token::Argument("a".to_string()),
                Token::Argument("a".to_string())
            ]
        );
    }

    #[test]
    fn test_push_whitespace_input_returns_single_whitespace_token() {
        let mut parser = LexicalParser::new();
        parser.push(' ');
        parser.push(' ');
        parser.push(' ');
        assert_eq!(parser.tokens, vec![Token::Whitespace]);
    }

    #[test]
    fn test_push_leading_whitespaces_are_ignored() {
        let mut parser = LexicalParser::new();
        parser.push(' ');
        parser.push(' ');
        parser.push('c');
        assert_eq!(parser.tokens, vec![Token::Command("c".to_string())]);
    }

    #[test]
    fn test_push_trailing_whitespaces_are_whitespace_tokens() {
        let mut parser = LexicalParser::new();
        parser.push('c');
        parser.push(' ');
        parser.push(' ');
        assert_eq!(
            parser.tokens,
            vec![Token::Command("c".to_string()), Token::Whitespace]
        );
    }

    #[test]
    fn test_push_in_between_whitespaces_are_ignored() {
        let mut parser = LexicalParser::new();
        parser.push('c');
        parser.push(' ');
        parser.push('a');
        assert_eq!(
            parser.tokens,
            vec![
                Token::Command("c".to_string()),
                Token::Argument("a".to_string())
            ]
        );
    }

    //////////////////////////////////////////////////
    // pop
    //////////////////////////////////////////////////

    #[test]
    fn test_pop_empty_parser_stays_empty() {
        let mut parser = LexicalParser::new();
        parser.pop();
        assert_eq!(parser.tokens, vec![]);
    }

    #[test]
    fn test_pop_removing_first_word_results_in_no_tokens() {
        let mut parser = LexicalParser::new();
        parser.push('c');
        parser.pop();
        assert_eq!(parser.tokens, vec![]);
    }

    #[test]
    fn test_pop_removing_other_word_removes_the_token() {
        let mut parser = LexicalParser::new();
        parser.push('c');
        parser.push(' ');
        parser.push('a');
        parser.pop();
        assert_eq!(parser.tokens, vec![Token::Command("c".to_string())]);
    }

    #[test]
    fn test_pop_removing_part_of_word_results_updates_the_token() {
        let mut parser = LexicalParser::new();
        parser.push('e');
        parser.push('x');
        parser.push('i');
        parser.push('t');
        parser.pop();
        assert_eq!(parser.tokens, vec![Token::Command("exi".to_string())]);
    }

    //////////////////////////////////////////////////
    // parse
    //////////////////////////////////////////////////

    #[test]
    fn test_parse_no_input_should_return_no_jobs() {
        let mut parser = LexicalParser::new();

        assert_eq!(parser.parse().jobs, vec![]);
    }

    #[test]
    fn test_parse_only_command_input_should_return_job_without_arguments() {
        let mut parser = LexicalParser::new();

        parser.push('l');
        parser.push('s');

        let result = parser.parse().jobs;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "ls");
        assert_eq!(result[0].arguments.len(), 0);
    }

    #[test]
    fn test_parse_command_and_arguments_input_should_return_job_with_arguments() {
        let mut parser = LexicalParser::new();

        parser.push('c');
        parser.push('d');
        parser.push(' ');
        parser.push('o');
        parser.push('s');

        let result = parser.parse().jobs;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "cd");
        assert_eq!(result[0].arguments, vec!["os"]);
    }

    #[test]
    fn test_parse_whitespace_tokens_are_ignored() {
        let mut parser = LexicalParser::new();

        parser.push('l');
        parser.push('s');
        parser.push(' ');

        let result = parser.parse().jobs;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "ls");
        assert_eq!(result[0].arguments.len(), 0);
    }
}
