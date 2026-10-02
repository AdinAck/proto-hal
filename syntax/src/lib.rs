//! The syntax of the MIOML language.

#![deny(missing_docs)]

pub mod ast;
pub mod delimiter;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod util;

use chumsky::{input::Input as _, prelude::*};

use crate::{
    ast::File,
    diagnostic::Diagnostic,
    util::{SourceId, Span},
};

/// Parse the source text of a file, [lexing](lexer), [balancing its delimiters](delimiter), and [parsing](parser)
/// it.
pub fn parse(text: &str, source: SourceId) -> (File<'_>, Vec<Diagnostic>) {
    let end = Span {
        source,
        start: text.len(),
        end: text.len(),
    };

    let tokens = lexer::lexer()
        .parse(text.with_context(source))
        .into_output()
        .expect("lexing never fails");

    let (tokens, mut diagnostics) = delimiter::balance(tokens, end);

    let (file, failures) = parser::parser()
        .parse(tokens.split_spanned(end))
        .into_output_errors();

    diagnostics.extend(
        failures
            .into_iter()
            .map(|failure| failure.into_diagnostic()),
    );

    (file.expect("file parsing is infallible"), diagnostics)
}

#[cfg(test)]
mod tests {
    use crate::{diagnostic::Kind, parse};

    #[test]
    fn sources() {
        for (s, expected, expected_diagnostics) in [
            (
                "import foo\n\nregister bar @ 0x0",
                "import foo\n\nregister bar @ 0x0",
                [].as_slice(),
            ),
            (
                "register foo { field bar",
                "register foo {\n    field bar\n}",
                [(Kind::Unclosed, "unclosed `{`")].as_slice(),
            ),
            (
                "register foo { field bar @ , }",
                "register foo {\n    field bar\n        <error>\n}",
                [(Kind::Unexpected, "expected value, found `,`")].as_slice(),
            ),
            (
                "register foo } { field bar @ ,",
                "register foo {\n    field bar\n        <error>\n}",
                [
                    (Kind::Unexpected, "unexpected `}`"),
                    (Kind::Unclosed, "unclosed `{`"),
                    (Kind::Unexpected, "expected value, found `,`"),
                ]
                .as_slice(),
            ),
        ] {
            let (file, diagnostics) = parse(s, 0);

            assert_eq!(
                file.to_string(),
                expected,
                "'{s}' should parse as '{expected}'"
            );
            assert_eq!(
                diagnostics
                    .iter()
                    .map(|diagnostic| &diagnostic.message)
                    .collect::<Vec<_>>(),
                expected_diagnostics
                    .iter()
                    .map(|(.., message)| message)
                    .collect::<Vec<_>>(),
                "'{s}' should produce the diagnostics of every stage",
            );
            assert!(
                diagnostics
                    .iter()
                    .zip(expected_diagnostics)
                    .all(|(diagnostic, (kind, ..))| diagnostic.is(*kind)),
                "'{s}' should produce diagnostics of the expected kinds",
            );
        }
    }

    #[test]
    fn source() {
        let (file, diagnostics) = parse("register foo {", 7);

        assert!(
            file.items.iter().all(|item| item.span.source == 7)
                && diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.primary_label.span.source == 7),
            "spans should point into the given source",
        );
    }
}
