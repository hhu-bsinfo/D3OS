use crate::located::Located;
use crate::located::Span;
use crate::located::locate;
use nom::Err;
use nom::error::Error;
use nom::error::ErrorKind;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{escaped, is_not, tag, take_till, take_while1},
    character::complete::{alphanumeric1, char, digit1, multispace1, one_of, satisfy},
    combinator::{map, not, peek, recognize},
    sequence::{preceded, separated_pair, terminated},
};

// use located_token to always know where a Token is from
pub type LToken<'s> = Located<Token<'s>, &'s str>;
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Token<'s> {
    Keyword(&'s str),
    Identifier(&'s str),
    Number(&'s str),
    String(&'s str),
    Operator(&'s str),
    Punctuation(char),
    Whitespace(&'s str),
    Comment(&'s str),
    Other(char),
}

pub fn match_any<'arr, 's>(
    tags: &'arr [&'s str],
) -> impl FnMut(&'s str) -> IResult<&'s str, &'s str> {
    move |input: &'s str| {
        for &kw in tags {
            if let Ok((rest, _)) = tag::<_, _, Error<&str>>(kw)(input) {
                return Ok((rest, kw));
            }
        }
        Err(Err::Error(Error::new(input, ErrorKind::Tag)))
    }
}

pub fn lex_clike<'a, 's>(input: &'s str, keywords: &'a [&'s str]) -> IResult<&'s str, LToken<'s>> {
    alt((
        locate(comment),
        locate(string),
        locate(whitespace),
        locate(|c| keyword(c, keywords)),
        locate(identifier),
        locate(number),
        locate(punctuation),
        locate(operator),
        locate(other),
    ))
    .parse(input)
}

fn keyword<'a, 's>(input: &'s str, keywords: &'a [&'s str]) -> IResult<&'s str, Token<'s>> {
    map(
        terminated(match_any(keywords), peek(not(alphanumeric1))),
        Token::Keyword,
    )
    .parse(input)
}

fn identifier(input: &str) -> IResult<&str, Token<'_>> {
    map(
        preceded(
            not(digit1),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        ),
        Token::Identifier,
    )
    .parse(input)
}

fn whitespace(input: &str) -> IResult<&str, Token<'_>> {
    map(multispace1, Token::Whitespace).parse(input)
}

fn number(input: &str) -> IResult<&str, Token<'_>> {
    map(
        recognize(alt((recognize((digit1, tag("."), digit1)), digit1))),
        Token::Number,
    )
    .parse(input)
}

fn punctuation(input: &str) -> IResult<&str, Token<'_>> {
    map(one_of("(){}[];,."), Token::Punctuation).parse(input)
}

fn string(input: &str) -> IResult<&str, Token<'_>> {
    map(
        recognize(separated_pair(
            char('\"'),
            escaped(is_not("\"\\"), '\\', one_of("\"ntr\\")),
            char('\"'),
        )),
        Token::String,
    )
    .parse(input)
}

fn comment(input: &str) -> IResult<&str, Token<'_>> {
    map(
        recognize(preceded(tag("//"), take_till(|c| c == '\n'))),
        Token::Comment,
    )
    .parse(input)
}
fn operator(input: &str) -> IResult<&str, Token<'_>> {
    const OPERATORS: &[&str] = &[
        "+", "-", "*", "/", "%", "&", "|", "^", "~", "!", "=", "<", ">", "+=", "-=", "*=", "/=",
        "%=", "&=", "|=", "^=", "<<", ">>", "++", "--", "==", "!=", "<=", ">=", "&&", "||",
    ];
    map(match_any(OPERATORS), Token::Operator).parse(input)
}

fn other(input: &str) -> IResult<&str, Token<'_>> {
    map(satisfy(|_| true), Token::Other).parse(input)
}

impl<'s> Located<Token<'s>, &'s str> {
    pub fn auto_span(&self, src: &'s str) -> Span {
        let len = match **self {
            Token::Keyword(s)
            | Token::Identifier(s)
            | Token::Number(s)
            | Token::String(s)
            | Token::Operator(s)
            | Token::Whitespace(s)
            | Token::Comment(s) => s.len(),

            Token::Punctuation(_) | Token::Other(_) => 1,
        };
        self.span(src, len)
    }
}

#[cfg(test)]
mod tests {
    use crate::{clike::lex_clike, located::Span};

    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn keyword_0() {
        let tags = &["int", "return", "if"];
        assert_eq!(keyword("int x", tags), Ok((" x", Token::Keyword("int"))));
    }
    #[test]
    fn keyword_prefix_int() {
        let tags = &["int", "return", "if"];
        assert_eq!(
            keyword("return42", tags),
            Err(nom::Err::Error(nom::error::Error::new(
                "42",
                nom::error::ErrorKind::Not
            )))
        ); // not followed by boundary
    }
    #[test]
    fn keyword_prefix_punc() {
        let tags = &["int", "return", "if"];
        assert_eq!(keyword("if(", tags), Ok(("(", Token::Keyword("if"))));
    }
    #[test]
    fn keyword_prefix_alph() {
        let tags = &["int", "return", "if"];
        assert!(keyword("introspect", tags).is_err());
    }

    #[test]
    fn identifier_0() {
        assert_eq!(
            identifier("ab01_cd23"),
            Ok(("", Token::Identifier("ab01_cd23")))
        );
    }

    #[test]
    fn identifier_start_digit() {
        assert!(identifier("0123abc").is_err());
    }

    #[test]
    fn identifier_1() {
        assert_eq!(identifier("a"), Ok(("", Token::Identifier("a"))));
    }

    #[test]
    fn number_0() {
        assert_eq!(number("123"), Ok(("", Token::Number("123"))));
    }

    #[test]
    fn number_1() {
        assert_eq!(number("3.14"), Ok(("", Token::Number("3.14"))));
    }

    #[test]
    fn number_2() {
        assert!(number("abc").is_err());
    }

    #[test]
    fn whitespace_0() {
        assert_eq!(whitespace(" "), Ok(("", Token::Whitespace(" "))));
    }

    #[test]
    fn whitespace_1() {
        assert_eq!(
            whitespace(" \r\n\t\n"),
            Ok(("", Token::Whitespace(" \r\n\t\n")))
        );
    }

    #[test]
    fn whitespace_2() {
        assert!(whitespace("a").is_err());
    }

    #[test]
    fn punctuation_0() {
        assert_eq!(punctuation(";"), Ok(("", Token::Punctuation(';'))));
    }

    #[test]
    fn punctuation_1() {
        assert_eq!(punctuation(") "), Ok((" ", Token::Punctuation(')'))));
    }

    #[test]
    fn punctuation_2() {
        assert_eq!(punctuation("{x"), Ok(("x", Token::Punctuation('{'))));
    }

    #[test]
    fn punctuation_3() {
        assert_eq!(punctuation("}else"), Ok(("else", Token::Punctuation('}'))));
    }

    #[test]
    fn punctuation_4() {
        assert!(punctuation("abc").is_err());
    }

    #[test]
    fn punctuation_5() {
        assert!(punctuation("").is_err());
    }

    #[test]
    fn lex_clike_0() {
        use Token::*;
        let input = "int main() {\n  int a = 3+4;\n  printf(\"%d\",a);\n  return 0;\n}";
        let mut rest = input;
        let keywords = &["int", "return"];

        let mut located_tokens = Vec::<Located<Token, &str>>::new();
        let mut tokens = Vec::<Token>::new();
        while !rest.is_empty() {
            match lex_clike(rest, keywords) {
                Ok((new_rest, token)) => {
                    located_tokens.push(token);
                    tokens.push(*token);
                    rest = new_rest;
                }
                Err(_) => break,
            }
        }

        let expected = [
            Keyword("int"),
            Whitespace(" "),
            Identifier("main"),
            Punctuation('('),
            Punctuation(')'),
            Whitespace(" "),
            Punctuation('{'),
            Whitespace("\n  "),
            Keyword("int"),
            Whitespace(" "),
            Identifier("a"),
            Whitespace(" "),
            Operator("="),
            Whitespace(" "),
            Number("3"),
            Operator("+"),
            Number("4"),
            Punctuation(';'),
            Whitespace("\n  "),
            Identifier("printf"),
            Punctuation('('),
            String("\"%d\""),
            Punctuation(','),
            Identifier("a"),
            Punctuation(')'),
            Punctuation(';'),
            Whitespace("\n  "),
            Keyword("return"),
            Whitespace(" "),
            Number("0"),
            Punctuation(';'),
            Whitespace("\n"),
            Punctuation('}'),
        ];
        assert_eq!(tokens, expected);
        assert_eq!(
            located_tokens[0].auto_span(input),
            Span { start: 0, end: 2 }
        );
        assert_eq!(
            located_tokens[1].auto_span(input),
            Span { start: 3, end: 3 }
        );
        assert_eq!(
            located_tokens[2].auto_span(input),
            Span { start: 4, end: 7 }
        );
    }

    #[test]
    fn string_0() {
        assert_eq!(
            string("\"{hello}[world]\""),
            Ok(("", Token::String("\"{hello}[world]\"")))
        );
    }

    #[test]
    fn string_1() {
        assert_eq!(
            string("\"he said: \\\"{ok}\\\"\""),
            Ok(("", Token::String("\"he said: \\\"{ok}\\\"\"")))
        );
    }

    #[test]
    fn string_2() {
        assert_eq!(
            string("\"line1\\nline2\\t{data}\""),
            Ok(("", Token::String("\"line1\\nline2\\t{data}\"")))
        );
    }

    #[test]
    fn string_3() {
        assert!(string("\"unterminated").is_err());
    }

    #[test]
    fn string_4() {
        assert!(string("\"bad\\xescape\"").is_err());
    }

    #[test]
    fn other() {
        let input = "@";
        let keywords = &[];
        let result = lex_clike(input, keywords);

        assert_eq!(
            result.unwrap().1,
            Token::Other('@'),
            "Expected @ to be parsed as Token::Char"
        );
    }
}
