use crate::located::Located;
use nom::Err;
use nom::character::is_newline;
use nom::character::is_space;
use nom::error::Error;
use nom::error::ErrorKind;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{alphanumeric1, digit1, multispace1, one_of},
    combinator::{map, not, peek, recognize},
    sequence::{preceded, separated_pair, terminated, tuple},
};

// use located_token to always know where a Token is from
pub type LToken<'s> = Located<Token<'s>, &'s str>;
#[derive(Debug, PartialEq)]
pub enum Token<'s> {
    Keyword(&'s str),
    Identifier(&'s str),
    Number(&'s str),
    Operator(&'s str),
    Punctuation(char),
    Whitespace(&'s str),
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

fn keyword<'a, 's>(input: &'s str, keywords: &'a [&'s str]) -> IResult<&'s str, Token<'s>> {
    map(
        terminated(match_any(keywords), peek(not(alphanumeric1))),
        Token::Keyword,
    )
    .parse(input)
}

fn identifier(input: &str) -> IResult<&str, Token> {
    map(
        preceded(
            not(digit1),
            take_while1(|c: char| c.is_alphanumeric() || c == '_'),
        ),
        Token::Identifier,
    )
    .parse(input)
}

pub fn whitespace(input: &str) -> IResult<&str, Token> {
    map(multispace1, Token::Whitespace).parse(input)
}

fn number(input: &str) -> IResult<&str, Token> {
    map(
        recognize(alt((
            recognize(tuple((digit1, tag("."), digit1))), // "1.23"
            digit1,                                       // "42"
        ))),
        Token::Number,
    )
    .parse(input)
}

fn punctuation(input: &str) -> IResult<&str, Token> {
    map(one_of("(){}[];,."), Token::Punctuation).parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
