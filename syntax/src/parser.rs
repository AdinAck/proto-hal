//! Transforming the tokens into an [abstract syntax tree](crate::ast).
//!
//! Each parser's documentation gives the syntax it accepts in
//! [EBNF](https://www.w3.org/TR/xml/#sec-notation).

mod entry;
mod failure;
mod iteration;
mod name;
mod path;
mod requirement;
mod set;
mod value;

use chumsky::{input::MappedInput, prelude::*};

pub use failure::Failure;

use crate::{
    ast::File,
    diagnostic::Expected,
    span::{Span, Spanned},
    token::{Ident, Literal, Token},
};

/// The parser input. The [lexed](crate::lexer) tokens with their spans.
pub type Input<'src> = MappedInput<'src, Token<'src>, Span, &'src [Spanned<Token<'src>>]>;

/// Error emitted by the parser.
pub type Error<'src> = extra::Err<Failure<'src>>;

/// The parser.
pub fn parser<'src>() -> impl Parser<'src, Input<'src>, File, Error<'src>> {
    end().to(File)
}

/// A [`Literal`].
fn literal<'src>() -> impl Parser<'src, Input<'src>, Literal, Error<'src>> + Clone {
    select! { Token::Literal(literal) => literal }.labelled(Expected::Literal)
}

/// An [`Ident`].
fn ident<'src>() -> impl Parser<'src, Input<'src>, Ident<'src>, Error<'src>> + Clone {
    select! { Token::Ident(ident) => ident }.labelled(Expected::Identifier)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use chumsky::prelude::*;

    use crate::{
        diagnostic::{Expected, Found},
        lexer::lexer,
        parser::{Error, Failure, parser, value::value},
        span::Span,
        token::{ident, token},
    };

    /// Lexes `s` and parses the produced tokens in their entirety.
    ///
    /// Intended for string literals in tests i.e. `parse(parser(), "foo")`. The output borrows from the tokens.
    pub(super) fn parse<T>(
        parser: impl Parser<'static, super::Input<'static>, T, Error<'static>>,
        s: &'static str,
    ) -> Result<T, Vec<Failure<'static>>> {
        let tokens = lexer()
            .parse(s.with_context(0))
            .into_result()
            .expect("lexer should be infallible")
            .leak();

        parser
            .parse(tokens.split_spanned(Span {
                source: 0,
                start: s.len(),
                end: s.len(),
            }))
            .into_result()
    }

    #[test]
    fn empty() {
        parse(parser(), "").unwrap();
    }

    #[test]
    fn padding() {
        parse(parser(), " // foo\n").unwrap();
    }

    #[test]
    fn reject() {
        assert_matches!(
            parse(parser(), "foo").unwrap_err().as_slice(),
            [Failure { found: Found::Token(ident!["foo"]), expected, .. }]
                if expected.iter().eq(&[Expected::End]),
            "'foo' should be unexpected where end of input is expected",
        );
    }

    #[test]
    fn labels() {
        assert_matches!(
            parse(value(), "@").unwrap_err().as_slice(),
            [Failure { found: Found::Token(token![@]), expected, .. }]
                if expected.iter().eq(&[Expected::Literal, Expected::Identifier]),
            "a value should expect a literal or an identifier",
        );
    }
}
