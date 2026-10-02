//! Failures emitted by the parser.

use chumsky::{DefaultExpected, error::Error, label::LabelError, util::MaybeRef};
use indexmap::IndexSet;

use crate::{
    diagnostic::{self, Diagnostic, Expected, Found},
    parser::Input,
    span::Span,
    token::Token,
};

/// A parse failure captures the list of expected symbols along with the erroneously encountered symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure<'src> {
    /// The erroneously encountered symbol.
    pub found: Found<'src>,
    /// The list of expected symbols.
    pub expected: IndexSet<Expected<'src>>,
    /// The span of the erroneously encountered symbol.
    pub span: Span,
}

impl Failure<'_> {
    /// Convert the failure into a [`Diagnostic`].
    pub fn into_diagnostic(self) -> Diagnostic {
        diagnostic::unexpected(self.span, self.found, &self.expected)
    }
}

impl<'src> Error<'src, Input<'src>> for Failure<'src> {
    fn merge(mut self, other: Self) -> Self {
        // note: `found` must be the same for both failures so no merge is needed

        self.expected.extend(other.expected);
        self
    }
}

impl<'src> LabelError<'src, Input<'src>, DefaultExpected<'src, Token<'src>>> for Failure<'src> {
    fn expected_found<E: IntoIterator<Item = DefaultExpected<'src, Token<'src>>>>(
        expected: E,
        found: Option<MaybeRef<'src, Token<'src>>>,
        span: Span,
    ) -> Self {
        Self {
            found: into_found(found),
            expected: expected
                .into_iter()
                .filter_map(|expected| match expected {
                    DefaultExpected::Token(token) => Some(Expected::Token(*token)),
                    DefaultExpected::EndOfInput | DefaultExpected::NothingElse => {
                        Some(Expected::End)
                    }
                    _ => None,
                })
                .collect(),
            span,
        }
    }
}

impl<'src> LabelError<'src, Input<'src>, Expected<'src>> for Failure<'src> {
    fn expected_found<E: IntoIterator<Item = Expected<'src>>>(
        expected: E,
        found: Option<MaybeRef<'src, Token<'src>>>,
        span: Span,
    ) -> Self {
        Self {
            found: into_found(found),
            expected: expected.into_iter().collect(),
            span,
        }
    }

    fn label_with(&mut self, label: Expected<'src>) {
        self.expected = IndexSet::from([label]);
    }
}

/// Convert what chumsky found into a [`Found`].
fn into_found<'src>(found: Option<MaybeRef<'src, Token<'src>>>) -> Found<'src> {
    match found {
        Some(token) => Found::Token(*token),
        None => Found::End,
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::{tests::parse, value::value};

    #[test]
    fn into_diagnostic() {
        let failures = parse(value(), "@").unwrap_err();

        let messages: Vec<_> = failures
            .into_iter()
            .map(|failure| failure.into_diagnostic().message)
            .collect();

        assert_eq!(
            messages,
            ["expected a literal or an identifier, found `@`"],
            "merged failures should produce one diagnostic naming every alternative",
        );
    }
}
