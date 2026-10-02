//! Diagnostics emitted while lexing, parsing, and validating.

use derive_more::Display;
use diagnostic::{Class, Code, Label, closest, disjunction};
use indexmap::IndexSet;

use crate::{
    ast::{Declaration, DeclarationKind, PropertyKind, Qualifier},
    token::{Keyword, Punctuation, Token},
    util::{Span, Spanned},
};

pub use diagnostic::Diagnostic;

/// A syntax diagnostic kind.
///
/// *Note: The associated [`Code`] of the diagnostic Kind is derived from the enum discriminant.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A token or end of input was encountered when not expected.
    Unexpected = 1,
    /// An opening delimiter has no closing delimiter.
    Unclosed,
    /// A declaration has a qualifier the declaration kind does not permit.
    UnpermittedQualifier,
    /// A declaration has a property the declaration kind does not permit.
    UnpermittedProperty,
    /// A declaration or interrupt table is placed where the enclosing module or declaration cannot contain it.
    Misplaced,
}

impl diagnostic::Kind for Kind {
    const CLASS: Class = Class::Syntax;

    fn code(self) -> Code {
        Code::new(self as u16)
    }
}

/// "unexpected {found}", or "expected {expected}, found {found}".
///
/// When the found identifier is likely a misspelling of a keyword the parser would accept, note: "a similarly named
/// keyword `{keyword}` exists".
///
/// When the found token is a file doc comment, note: "file doc comments can only appear at the start of a file".
pub fn unexpected(
    offending: Span,
    found: Found,
    expected: &IndexSet<Expected>,
    keywords: &IndexSet<Keyword>,
) -> Diagnostic {
    let message = match expected.is_empty() {
        true => format!("unexpected {found}"),
        false => format!("expected {}, found {found}", disjunction(expected)),
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

/// "unclosed `{opener}`".
pub fn unclosed(opener: Spanned<Punctuation>, closer: Spanned<Punctuation>) -> Diagnostic {
    Diagnostic::error(
        Kind::Unclosed,
        format!("unclosed `{}`", *opener),
        Label::new(format!("expected `{}` here", *closer), closer.span),
    )
    .supporting_label(Label::new("unclosed delimiter", opener.span))
}

/// "unexpected qualifier `{qualifier}` on {kind} `{name}`".
///
/// note: "expected one of {permitted}", "expected {permitted}" when one is permitted, or "{kind}s have no qualifiers"
/// when none are permitted.
pub fn unpermitted_qualifier(
    declaration: &Declaration,
    qualifier: Spanned<Qualifier>,
    permitted: impl IntoIterator<Item = Qualifier>,
) -> Diagnostic {
    Diagnostic::error(
        Kind::UnpermittedQualifier,
        format!(
            "unexpected qualifier `{}` on {} `{}`",
            *qualifier, *declaration.kind, *declaration.name
        ),
        Label::new("unexpected qualifier here", qualifier.span),
    )
    .note(expectation(
        permitted
            .into_iter()
            .map(|qualifier| format!("`{qualifier}`")),
        format!("{}s have no qualifiers", *declaration.kind),
    ))
}

/// "unexpected property `{property}` on {kind} `{name}`".
///
/// note: "expected one of {permitted}", or "expected {permitted}" when one is permitted.
///
/// When the property is `requires` on a variant of a schema, note: "statewise entitlements may be imposed only by
/// variants of fields".
pub fn unpermitted_property(
    declaration: &Declaration,
    property: Spanned<PropertyKind>,
    ancestor: Option<DeclarationKind>,
    permitted: impl IntoIterator<Item = PropertyKind>,
) -> Diagnostic {
    let diagnostic = Diagnostic::error(
        Kind::UnpermittedProperty,
        format!(
            "unexpected property `{}` on {} `{}`",
            *property, *declaration.kind, *declaration.name
        ),
        Label::new("unexpected property here", property.span),
    )
    .note(expectation(
        permitted.into_iter().map(|kind| format!("`{kind}`")),
        format!("{}s have no properties", *declaration.kind),
    ));

    match (*declaration.kind, ancestor, *property) {
        (DeclarationKind::Variant, Some(DeclarationKind::Schema), PropertyKind::Requires(None)) => {
            diagnostic.note("statewise entitlements may be imposed only by variants of fields")
        }
        _ => diagnostic,
    }
}

/// "unexpected {kind} `{name}` in {parent kind} `{parent}`", or at module level "unexpected {kind} `{name}`".
///
/// note: "children of {parent kind}s may be {permitted}", "{parent kind}s have no children" when none are permitted,
/// or at module level "module items may be {permitted}", where the children permitted in a device include interrupt
/// tables.
pub fn misplaced_declaration(
    declaration: &Declaration,
    parent: Option<&Declaration>,
    permitted: impl IntoIterator<Item = DeclarationKind>,
) -> Diagnostic {
    let message = match parent {
        Some(parent) => format!(
            "unexpected {} `{}` in {} `{}`",
            *declaration.kind, *declaration.name, *parent.kind, *parent.name
        ),
        None => format!("unexpected {} `{}`", *declaration.kind, *declaration.name),
    };

    misplaced(
        message,
        Label::new("unexpected declaration here", declaration.kind.span),
        parent,
        permitted,
    )
}

/// "unexpected interrupt table in {parent kind} `{parent}`".
///
/// note: "children of {parent kind}s may be {permitted}", or "{parent kind}s have no children" when none are permitted.
pub fn misplaced_interrupts(
    interrupts: Span,
    parent: &Declaration,
    permitted: impl IntoIterator<Item = DeclarationKind>,
) -> Diagnostic {
    misplaced(
        format!(
            "unexpected interrupt table in {} `{}`",
            *parent.kind, *parent.name
        ),
        Label::new("unexpected interrupt table here", interrupts),
        Some(parent),
        permitted,
    )
}

/// Create a misplacement diagnostic with the provided message and primary label, labelling the provided parent, if
/// any, and noting which children the parent may contain.
fn misplaced(
    message: String,
    primary_label: Label,
    parent: Option<&Declaration>,
    permitted: impl IntoIterator<Item = DeclarationKind>,
) -> Diagnostic {
    let interrupts = parent
        .is_some_and(|parent| *parent.kind == DeclarationKind::Device)
        .then_some("interrupt tables".to_string());

    let children = permitted
        .into_iter()
        .map(|kind| format!("{kind}s"))
        .chain(interrupts)
        .collect::<Vec<_>>();

    let note = match (parent, children.is_empty()) {
        (None, ..) => format!("module items may be {}", disjunction(children)),
        (Some(parent), true) => format!("{}s have no children", *parent.kind),
        (Some(parent), false) => format!(
            "children of {}s may be {}",
            *parent.kind,
            disjunction(children)
        ),
    };

    let diagnostic = Diagnostic::error(Kind::Misplaced, message, primary_label).note(note);

    match parent {
        Some(parent) => diagnostic.supporting_label(Label::new(
            format!("parent {}", *parent.kind),
            parent.name.span,
        )),
        None => diagnostic,
    }
}

/// The note stating which of the provided entries were expected, such as "expected one of `@` or `reset`", or the
/// provided statement when no entries were expected.
fn expectation(entries: impl IntoIterator<Item = String>, none: String) -> String {
    let entries = entries.into_iter().collect::<Vec<_>>();

    match entries.as_slice() {
        [] => none,
        [entry] => format!("expected {entry}"),
        _ => format!("expected one of {}", disjunction(entries)),
    }
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
    /// A [value](crate::ast::Value), or a [list](crate::ast::List) of values, after `@`, `~`, `reset`, or `wordsize`.
    #[display("value")]
    Value,
    /// A [set](crate::ast::Set), after `access`.
    #[display("set")]
    Set,
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

/// Produce a description of the token for use in diagnostic messages, such as "`foo`" or "doc comment".
fn describe(token: &Token) -> String {
    match token {
        Token::Doc(..) => "doc comment".to_string(),
        Token::FileDoc(..) => "file doc comment".to_string(),
        token => format!("`{token}`"),
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
        similar_asserts::assert_eq!(
            expected: "E1001",
            found: unexpected(DUMMY_SPAN, Found::End, &IndexSet::new(), &IndexSet::new())
                .id
                .to_string(),
            "expected the `unexpected` diagnostic to be identified as `E1001`",
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
                [Expected::Set].as_slice(),
                "expected set, found end of input",
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

            similar_asserts::assert_eq!(
                expected: message,
                found: diagnostic.message,
                "expected the message of the `unexpected` diagnostic to name {found}, and each expectation in \
                 {expected:?}",
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

        similar_asserts::assert_eq!(
            expected: "E1002",
            found: diagnostic.id.to_string(),
            "expected the `unclosed` diagnostic to be identified as `E1002`",
        );
        similar_asserts::assert_eq!(
            expected: "unclosed `{`",
            found: diagnostic.message,
            "expected the message of the `unclosed` diagnostic to name the opening delimiter",
        );
        similar_asserts::assert_eq!(
            expected: Label::new("expected `}` here", end),
            found: diagnostic.primary_label,
            "expected the primary label of the `unclosed` diagnostic to span where the closing delimiter was expected",
        );
        similar_asserts::assert_eq!(
            expected: vec![Label::new("unclosed delimiter", opening)],
            found: diagnostic.supporting_labels,
            "expected the supporting label of the `unclosed` diagnostic to span the opening delimiter",
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

            similar_asserts::assert_eq!(
                expected: Vec::from_iter(note),
                found: diagnostic.notes,
                "expected the `unexpected` diagnostic for `{found}` to note a near miss only of a keyword listed in \
                 {keywords:?}",
            );
        }
    }

    #[test]
    fn file_doc() {
        similar_asserts::assert_eq!(
            expected: vec!["file doc comments can only appear at the start of a file"],
            found: unexpected(
                DUMMY_SPAN,
                Found::Token(Token::FileDoc(Doc("foo"))),
                &IndexSet::new(),
                &IndexSet::new(),
            )
            .notes,
            "expected the `unexpected` diagnostic for a file doc comment to note where file doc comments may appear",
        );
    }
}
