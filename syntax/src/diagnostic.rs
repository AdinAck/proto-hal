//! Diagnostics emitted while lexing and parsing.

use derive_more::Display;
use diagnostic::{Class, Code, Label};
use indexmap::IndexSet;

use crate::{span::Span, token::Token};

/// A syntax diagnostic.
pub type Diagnostic = diagnostic::Diagnostic<Kind>;

/// A syntax diagnostic kind.
///
/// *Note: A kind's discriminant is its code.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A token, or the end of the input, where the grammar does not allow it.
    Unexpected = 1,
}

impl diagnostic::Kind for Kind {
    const CLASS: Class = Class::Syntax;

    fn code(self) -> Code {
        Code::new(self as u16)
    }
}

/// "expected a literal or `foo`, found `bar`".
pub fn unexpected(offending: Span, found: Found, expected: &IndexSet<Expected>) -> Diagnostic {
    let message = match expected.is_empty() {
        true => format!("unexpected {found}"),
        false => format!("expected {}, found {found}", expected_list(expected)),
    };

    let label = match found {
        Found::Token(..) => "unexpected token",
        Found::End => "end of input here",
    };

    Diagnostic::error(Kind::Unexpected, message, Label::new(label, offending))
}

/// The symbol erroneously encountered while parsing. Implements [`Display`](core::fmt::Display)
/// appropriately for rendering in a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Found<'src> {
    /// A token.
    #[display("{}", describe(_0))]
    Token(Token<'src>),
    /// The end of the input.
    #[display("end of input")]
    End,
}

/// The symbol expected by the parser when encountering an erroneous one. Implements [`Display`](core::fmt::Display)
/// appropriately for rendering in a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
pub enum Expected<'src> {
    /// A particular token.
    #[display("{}", describe(_0))]
    Token(Token<'src>),
    /// Any [literal](crate::token::Literal).
    #[display("a literal")]
    Literal,
    /// Any [identifier](crate::token::Ident).
    #[display("an identifier")]
    Identifier,
    /// An iteration variable, after `#`.
    #[display("an iteration variable")]
    IterationVariable,
    /// A segment of a path delineated by `.`.
    #[display("a path segment")]
    PathSegment,
    /// The end of the input.
    #[display("end of input")]
    End,
}

/// Produce a description of the token for use in diagnostic messages i.e. "`foo`", or as "a doc comment".
fn describe(token: &Token) -> String {
    match token {
        Token::Doc(..) => "a doc comment".to_string(),
        token => format!("`{token}`"),
    }
}

/// List of expected symbols rendered in a message i.e. "`,`, `]`, or a literal".
fn expected_list(expected: &IndexSet<Expected>) -> String {
    let names: Vec<_> = expected.iter().map(ToString::to_string).collect();

    match names.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexSet;

    use crate::{
        diagnostic::{Expected, Found, unexpected},
        span::Span,
        token::token,
    };

    const SPAN: Span = Span {
        source: 0,
        start: 0,
        end: 1,
    };

    #[test]
    fn id() {
        assert_eq!(
            unexpected(SPAN, Found::End, &IndexSet::new())
                .id()
                .to_string(),
            "E1001",
            "id doesn't match expected"
        );
    }

    #[test]
    fn messages() {
        for (diagnostic, expected) in [
            (
                unexpected(SPAN, Found::End, &IndexSet::new()),
                "unexpected end of input",
            ),
            (
                unexpected(SPAN, Found::Token(token![,]), &IndexSet::new()),
                "unexpected `,`",
            ),
            (
                unexpected(SPAN, Found::End, &IndexSet::from([Expected::Literal])),
                "expected a literal, found end of input",
            ),
            (
                unexpected(
                    SPAN,
                    Found::Token(token![,]),
                    &IndexSet::from([Expected::Literal, Expected::Token(token![RBracket])]),
                ),
                "expected a literal or `]`, found `,`",
            ),
            (
                unexpected(
                    SPAN,
                    Found::Token(token![@]),
                    &IndexSet::from([Expected::Literal, Expected::Identifier, Expected::End]),
                ),
                "expected a literal, an identifier, or end of input, found `@`",
            ),
            (
                unexpected(
                    SPAN,
                    Found::Token(token![#]),
                    &IndexSet::from([Expected::IterationVariable]),
                ),
                "expected an iteration variable, found `#`",
            ),
            (
                unexpected(SPAN, Found::End, &IndexSet::from([Expected::PathSegment])),
                "expected a path segment, found end of input",
            ),
        ] {
            assert_eq!(
                diagnostic.message, expected,
                "message doesn't match expected"
            );
        }
    }
}
