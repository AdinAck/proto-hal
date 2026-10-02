//! Diagnostics emitted while lexing and parsing.

use derive_more::Display;
use diagnostic::{Class, Code, Label, closest};
use indexmap::IndexSet;

use crate::{
    token::{Keyword, Punctuation, Token},
    util::{Span, Spanned},
};

pub use diagnostic::Diagnostic;

/// A syntax diagnostic kind.
///
/// *Note: The associated [`Code`] of the diagnostic Kind is derived from the enum discriminant.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A token or end of input was encoutered when not expected.
    Unexpected = 1,
    /// An opening delimiter has no closing delimiter.
    Unclosed,
}

impl diagnostic::Kind for Kind {
    const CLASS: Class = Class::Syntax;

    fn code(self) -> Code {
        Code::new(self as u16)
    }
}

/// "expected literal or `foo`, found `bar`".
pub fn unexpected(
    offending: Span,
    found: Found,
    expected: &IndexSet<Expected>,
    keywords: &IndexSet<Keyword>,
) -> Diagnostic {
    let message = match expected.is_empty() {
        true => format!("unexpected {found}"),
        false => format!("expected {}, found {found}", expected_list(expected)),
    };

    let label = match found {
        Found::Token(..) => "unexpected token",
        Found::End => "end of input here",
    };

    let diagnostic = Diagnostic::error(Kind::Unexpected, message, Label::new(label, offending));

    let diagnostic = match near_miss(found, keywords) {
        Some(keyword) => diagnostic.note(format!("a similarly named keyword `{keyword}` exists")),
        None => diagnostic,
    };

    match found {
        Found::Token(Token::FileDoc(..)) => {
            diagnostic.note("file doc comments can only appear at the start of a file")
        }
        _ => diagnostic,
    }
}

/// "unclosed `{`".
pub fn unclosed(opener: Spanned<Punctuation>, closer: Spanned<Punctuation>) -> Diagnostic {
    Diagnostic::error(
        Kind::Unclosed,
        format!("unclosed `{}`", *opener),
        Label::new(format!("expected `{}` here", *closer), closer.span),
    )
    .supporting_label(Label::new("unclosed delimiter", opener.span))
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
    #[display("literal")]
    Literal,
    /// Any [identifier](crate::token::Ident).
    #[display("identifier")]
    Identifier,
    /// An iteration variable, after `#`.
    #[display("iteration variable")]
    IterationVariable,
    /// A [name](crate::ast::Name), after a declaration's kind.
    #[display("name")]
    Name,
    /// A [segment](crate::ast::PathSegment) of a [path](crate::ast::Path).
    #[display("path segment")]
    PathSegment,
    /// A [value](crate::ast::Value), or a [list](crate::ast::List) of values, after `@`, `~`, or `reset`.
    #[display("value")]
    Value,
    /// A [path](crate::ast::ModulePath) to a module or a definition, after `import`, `assumes`, or `refines`.
    #[display("module path")]
    ModulePath,
    /// An [entitlement space](crate::ast::EntitlementSpace), after `requires`.
    #[display("entitlement space")]
    EntitlementSpace,
    /// A [qualifier](crate::ast::Qualifier), before a declaration's kind.
    #[display("qualifier")]
    Qualifier,
    /// A [property](crate::ast::Property), after a declaration's name.
    #[display("property")]
    Property,
    /// A [declaration](crate::ast::Declaration).
    #[display("declaration")]
    Declaration,
    /// An [interrupt](crate::ast::Interrupt), within an [interrupt table](crate::ast::InterruptTable).
    #[display("interrupt")]
    Interrupt,
    /// The end of the input.
    #[display("end of input")]
    End,
}

/// Search among the provided list of applicable keywords for a potential misspelling.
fn near_miss(found: Found, keywords: &IndexSet<Keyword>) -> Option<String> {
    let Found::Token(Token::Ident(ident)) = found else {
        return None;
    };

    closest(&ident, keywords.iter().map(ToString::to_string))
}

/// Produce a description of the token for use in diagnostic messages i.e. "`foo`", or "doc comment".
fn describe(token: &Token) -> String {
    match token {
        Token::Doc(..) => "doc comment".to_string(),
        Token::FileDoc(..) => "file doc comment".to_string(),
        token => format!("`{token}`"),
    }
}

/// List of expected symbols rendered in a message i.e. "`,`, `]`, or literal".
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
    use ::diagnostic::Label;
    use indexmap::IndexSet;

    use crate::{
        diagnostic::{Expected, Found, unclosed, unexpected},
        token::{Doc, Keyword, LBrace, RBrace, Token, ident, token},
        util::{Span, Spanned},
    };

    const DUMMY_SPAN: Span = Span {
        source: 0,
        start: 0,
        end: 1,
    };

    #[test]
    fn id() {
        assert_eq!(
            unexpected(DUMMY_SPAN, Found::End, &IndexSet::new(), &IndexSet::new())
                .id
                .to_string(),
            "E1001",
            "id doesn't match expected"
        );
    }

    #[test]
    fn messages() {
        for (found, expected, message) in [
            (Found::End, [].as_slice(), "unexpected end of input"),
            (Found::Token(token![,]), [].as_slice(), "unexpected `,`"),
            (
                Found::Token(Token::FileDoc(Doc("foo"))),
                [].as_slice(),
                "unexpected file doc comment",
            ),
            (
                Found::End,
                [Expected::Literal].as_slice(),
                "expected literal, found end of input",
            ),
            (
                Found::Token(token![,]),
                [Expected::Literal, Expected::Token(token![RBracket])].as_slice(),
                "expected literal or `]`, found `,`",
            ),
            (
                Found::Token(token![@]),
                [Expected::Literal, Expected::Identifier, Expected::End].as_slice(),
                "expected literal, identifier, or end of input, found `@`",
            ),
            (
                Found::Token(token![#]),
                [Expected::IterationVariable].as_slice(),
                "expected iteration variable, found `#`",
            ),
            (
                Found::End,
                [Expected::PathSegment].as_slice(),
                "expected path segment, found end of input",
            ),
            (
                Found::Token(token![,]),
                [Expected::Value].as_slice(),
                "expected value, found `,`",
            ),
            (
                Found::End,
                [Expected::ModulePath].as_slice(),
                "expected module path, found end of input",
            ),
            (
                Found::Token(token![@]),
                [Expected::EntitlementSpace].as_slice(),
                "expected entitlement space, found `@`",
            ),
            (
                Found::Token(token![@]),
                [Expected::Name].as_slice(),
                "expected name, found `@`",
            ),
            (
                Found::Token(token![,]),
                [
                    Expected::Property,
                    Expected::Token(token![LBrace]),
                    Expected::Declaration,
                    Expected::Token(token![RBrace]),
                ]
                .as_slice(),
                "expected property, `{`, declaration, or `}`, found `,`",
            ),
            (
                Found::Token(token![@]),
                [Expected::Interrupt, Expected::Token(token![RBrace])].as_slice(),
                "expected interrupt or `}`, found `@`",
            ),
        ] {
            let diagnostic = unexpected(
                DUMMY_SPAN,
                found,
                &expected.iter().copied().collect(),
                &IndexSet::new(),
            );

            assert_eq!(
                diagnostic.message, message,
                "message doesn't match expected"
            );
        }
    }

    #[test]
    fn labels() {
        let opening = Span {
            source: 0,
            start: 13,
            end: 14,
        };
        let end = Span {
            source: 0,
            start: 24,
            end: 24,
        };

        let diagnostic = unclosed(
            Spanned {
                inner: LBrace,
                span: opening,
            },
            Spanned {
                inner: RBrace,
                span: end,
            },
        );

        assert_eq!(
            diagnostic.id.to_string(),
            "E1002",
            "id doesn't match expected"
        );
        assert_eq!(
            diagnostic.message, "unclosed `{`",
            "message doesn't match expected"
        );
        assert_eq!(
            diagnostic.primary_label,
            Label::new("expected `}` here", end),
            "the primary label should be where the closing delimiter was expected",
        );
        assert_eq!(
            diagnostic.supporting_labels,
            [Label::new("unclosed delimiter", opening)],
            "a supporting label should be at the opening delimiter",
        );
    }

    #[test]
    fn near_misses() {
        for (found, keywords, note) in [
            (
                "reed",
                [Keyword::Read, Keyword::Write, Keyword::Register].as_slice(),
                Some("a similarly named keyword `read` exists"),
            ),
            (
                "regster",
                [Keyword::Register, Keyword::Field].as_slice(),
                Some("a similarly named keyword `register` exists"),
            ),
            ("foo", [Keyword::Register, Keyword::Field].as_slice(), None),
            ("reed", [].as_slice(), None),
        ] {
            let diagnostic = unexpected(
                DUMMY_SPAN,
                Found::Token(ident![found]),
                &IndexSet::new(),
                &keywords.iter().copied().collect(),
            );

            assert_eq!(
                diagnostic.notes,
                Vec::from_iter(note),
                "'{found}' should be noted as a near miss only of a keyword the parser would accept",
            );
        }
    }

    #[test]
    fn file_doc() {
        assert_eq!(
            unexpected(
                DUMMY_SPAN,
                Found::Token(Token::FileDoc(Doc("foo"))),
                &IndexSet::new(),
                &IndexSet::new(),
            )
            .notes,
            ["file doc comments can only appear at the start of a file"],
            "a misplaced file doc comment should be noted with where it may appear",
        );
    }
}
