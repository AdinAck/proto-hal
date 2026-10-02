//! Transforming the tokens into an [abstract syntax tree](crate::ast).
//!
//! Each parser's documentation gives the syntax it accepts in
//! [EBNF](https://www.w3.org/TR/xml/#sec-notation).

mod declaration;
mod entitlement;
mod failure;
mod import;
mod interrupt;
mod iteration;
mod list;
mod name;
mod path;
mod property;
mod recovery;
mod set;
mod value;

use chumsky::{input::MappedInput, prelude::*};

use declaration::declaration;
use import::import;
use recovery::{parsed, skipped};

pub use failure::Failure;

use crate::{
    ast::{File, Item},
    diagnostic::Expected,
    token::{Doc, Ident, Literal, Token},
    util::{Span, Spanned},
};

/// The parser input. The [lexed](crate::lexer) tokens with their spans.
pub type Input<'tokens, 'src> =
    MappedInput<'tokens, Token<'src>, Span, &'tokens [Spanned<Token<'src>>]>;

/// Error emitted by the parser.
// note: boxed to keep failures small, as chumsky moves them between parsers
pub type Error<'src> = extra::Err<Box<Failure<'src>>>;

/// The parser.
///
/// ```text
/// file ::= FILE_DOC* (import | declaration)*
/// ```
pub fn parser<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, File<'src>, Error<'src>> {
    let item = choice((
        import().map(Item::Import),
        declaration()
            .labelled(Expected::Declaration)
            .map(Item::Declaration),
    ));

    file_doc()
        .spanned()
        .repeated()
        .collect()
        .then(parsed(item, skipped(), end()).repeated().collect())
        .then_ignore(end())
        .map(|(docs, items)| File { docs, items })
}

/// A [`Literal`].
fn literal<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Literal<'src>, Error<'src>> + Clone {
    select! { Token::Literal(literal) => literal }.labelled(Expected::Literal)
}

/// An [`Ident`].
fn ident<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Ident<'src>, Error<'src>> + Clone {
    select! { Token::Ident(ident) => ident }.labelled(Expected::Identifier)
}

/// A [`Doc`].
fn doc<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Doc<'src>, Error<'src>> + Clone {
    select! { Token::Doc(doc) => doc }
}

/// A [`Doc`] documenting the file.
fn file_doc<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Doc<'src>, Error<'src>> + Clone {
    select! { Token::FileDoc(doc) => doc }
}

#[cfg(test)]
mod tests {
    use ::diagnostic::Diagnostic;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{Failure, value::value},
        token::token,
    };

    #[test]
    fn files() {
        for s in [
            "",
            "import foo",
            "import foo\nimport bar::baz as qux",
            "import foo\n\nschema bar {\n    variant Baz ~ 0\n}",
            "schema foo {\n    variant Bar ~ 0\n}\n\nimport baz\n\ndevice qux",
            "/// Foo.\ndevice foo\n\nperipheral bar @ 0x0",
            "//! Foo.",
            "//! Foo.\n//! Bar.\n\nimport baz",
            "//! Foo.\n\n/// Bar.\ndevice baz",
            "//! Foo.\n//!\n//! Bar.\n\n/// Baz.\n///\n/// Qux.\ndevice quux",
        ] {
            let (file, diagnostics) = crate::parse(s, 0);

            similar_asserts::assert_eq!(
                expected: s,
                found: file.to_string(),
                "expected display impl for the file parsed from {s:?} to reproduce the source text",
            );
            similar_asserts::assert_eq!(
                expected: Vec::<&str>::new(),
                found: messages(&diagnostics),
                "expected parsing {s:?} to emit no diagnostics",
            );
        }
    }

    #[test]
    fn padding() {
        let s = " // foo\n";

        let (file, diagnostics) = crate::parse(s, 0);

        similar_asserts::assert_eq!(
            expected: (String::new(), Vec::new()),
            found: (file.to_string(), messages(&diagnostics)),
            "expected parsing {s:?} to produce an empty file, and emit no diagnostics",
        );
    }

    #[test]
    fn reject() {
        for (s, message) in [
            ("@", "expected `import` or declaration, found `@`"),
            (
                "import foo\n//! Bar.",
                "expected `::`, `as`, `import`, or declaration, found file doc comment",
            ),
        ] {
            let (.., diagnostics) = crate::parse(s, 0);

            similar_asserts::assert_eq!(
                expected: vec![message],
                found: messages(&diagnostics),
                "expected parsing {s:?} to emit a single diagnostic",
            );
        }
    }

    #[test]
    fn labels() {
        similar_asserts::assert_eq!(
            expected: vec![(Found::Token(token![@]), vec![Expected::Literal, Expected::Identifier])],
            found: expectations(parse!(value(), "@").unwrap_err()),
            "expected parsing `@` as a value to fail, expecting a literal or an identifier",
        );
    }

    /// A vector containing the messages of the provided diagnostics, in order.
    fn messages(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    /// A vector containing the found symbol and the expected symbols of each of the provided failures, in order.
    pub(super) fn expectations<'src>(
        failures: impl IntoIterator<Item = Failure<'src>>,
    ) -> Vec<(Found<'src>, Vec<Expected<'src>>)> {
        failures
            .into_iter()
            .map(|failure| (failure.found, failure.expected.into_iter().collect()))
            .collect()
    }

    /// Lex the provided source text and parse the produced tokens in their entirety with the provided parser, producing
    /// the output, or the failures.
    macro_rules! parse {
        ($parser:expr, $s:expr) => {{
            use ::chumsky::{Parser as _, input::Input as _};

            let s: &str = $s;
            let tokens = $crate::lexer::lexer()
                .parse(s.with_context(0))
                .into_output()
                .expect("lexing never fails");

            $parser
                .parse(tokens.split_spanned($crate::util::Span {
                    source: 0,
                    start: s.len(),
                    end: s.len(),
                }))
                .into_result()
                .map_err(|failures| {
                    failures
                        .into_iter()
                        .map(|failure| *failure)
                        .collect::<Vec<_>>()
                })
        }};
    }

    pub(super) use parse;
}
