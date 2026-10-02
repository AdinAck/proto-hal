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
    use std::collections::BTreeSet;

    use ::diagnostic::Kind as _;

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

            similar_asserts::assert_eq!(
                expected: expected,
                found: file.to_string(),
                "expected the file parsed from {s:?} to display as {expected:?}",
            );
            similar_asserts::assert_eq!(
                expected: expected_diagnostics
                    .iter()
                    .map(|&(kind, message)| (kind.code(), message))
                    .collect::<Vec<_>>(),
                found: diagnostics
                    .iter()
                    .map(|diagnostic| (diagnostic.id.code, diagnostic.message.as_str()))
                    .collect::<Vec<_>>(),
                "expected parsing {s:?} to emit the diagnostics of each stage, in order",
            );
        }
    }

    #[test]
    fn source() {
        let (file, diagnostics) = parse("register foo {", 7);

        similar_asserts::assert_eq!(
            expected: BTreeSet::from([7]),
            found: file
                .items
                .iter()
                .map(|item| item.span.source)
                .chain(diagnostics.iter().map(|diagnostic| diagnostic.primary_label.span.source))
                .collect::<BTreeSet<_>>(),
            "expected the spans produced by parsing source `7` to point into source `7`",
        );
    }
}
