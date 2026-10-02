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
// note: boxed to keep failures small, as chumsky moves them through every parser
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
    use std::assert_matches;

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

            assert_eq!(file.to_string(), s, "'{s}' should display as written");
            assert!(
                diagnostics.is_empty(),
                "'{s}' should parse without diagnostics"
            );
        }
    }

    #[test]
    fn padding() {
        let (file, diagnostics) = crate::parse(" // foo\n", 0);

        assert!(
            file.items.is_empty() && diagnostics.is_empty(),
            "padding alone should parse as an empty file",
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

            assert_eq!(
                diagnostics
                    .iter()
                    .map(|diagnostic| &diagnostic.message)
                    .collect::<Vec<_>>(),
                [message],
                "'{s}' should be rejected",
            );
        }
    }

    #[test]
    fn labels() {
        assert_matches!(
            parse!(value(), "@").unwrap_err().as_slice(),
            [Failure { found: Found::Token(token![@]), expected, .. }]
                if expected.iter().eq(&[Expected::Literal, Expected::Identifier]),
            "a value should expect a literal or an identifier",
        );
    }

    /// Lex `s` and parse the produced tokens in their entirety with `parser`, producing the output, or every failure.
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
