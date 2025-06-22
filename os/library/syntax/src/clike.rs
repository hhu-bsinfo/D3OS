use crate::located::Located;
use nom::Err;
use nom::error::Error;
use nom::error::ErrorKind;
use nom::{
    IResult, Parser,
    bytes::complete::{tag, take_while1},
    character::complete::{alphanumeric1, digit1},
    combinator::{map, not, peek},
    sequence::{preceded, terminated},
};

// use located_token to always know where a Token is from
pub type LToken<'s> = Located<Token<'s>, &'s str>;
#[derive(Debug, PartialEq)]
pub enum Token<'s> {
    Keyword(&'s str),
    Identifier(&'s str),
    Number(&'s str),
    Operator(&'s str),
    Punctuation(&'s str),
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

fn identifier(input: &str) -> IResult<&str, &str> {
    preceded(
        not(digit1),
        take_while1(|c: char| c.is_alphanumeric() || c == '_'),
    )
    .parse(input)
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
}
