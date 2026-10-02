//! Failures emitted by the parser.

use chumsky::{DefaultExpected, error::Error, label::LabelError, util::MaybeRef};
use indexmap::IndexSet;

use crate::{
    diagnostic::{self, Diagnostic, Expected, Found},
    parser::Input,
    token::{Keyword, Token},
    util::Span,
};

/// A parse failure captures the set of expected symbols along with the erroneously encountered symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure<'src> {
    /// The erroneously encountered symbol.
    pub found: Found<'src>,
    /// The list of expected symbols.
    pub expected: IndexSet<Expected<'src>>,
    /// The keywords attempted by previous failed parsers, so that near misses only suggest contextually applicable
    /// keywords.
    ///
    /// *Note: Attempted keywords are retained even during a re-labelling, to keep a record of the applicable keywords.*
    pub keywords: IndexSet<Keyword>,
    /// The span of the erroneously encountered symbol.
    pub span: Span,
}

impl<'src> Failure<'src> {
    /// Create a failure.
    fn new(found: Found<'src>, expected: IndexSet<Expected<'src>>, span: Span) -> Self {
        let keywords = expected
            .iter()
            .filter_map(|expected| match expected {
                Expected::Token(Token::Keyword(keyword)) => Some(*keyword),
                _ => None,
            })
            .collect();

        Self {
            found,
            expected,
            keywords,
            span,
        }
    }

    /// Convert the failure into a [`Diagnostic`].
    pub fn into_diagnostic(self) -> Diagnostic {
        diagnostic::unexpected(self.span, self.found, &self.expected, &self.keywords)
    }
}

impl<'tokens, 'src> Error<'tokens, Input<'tokens, 'src>> for Box<Failure<'src>> {
    fn merge(mut self, other: Self) -> Self {
        // note: `found` must be the same for both failures so no merge is needed

        self.expected.extend(other.expected);
        self.keywords.extend(other.keywords);
        self
    }
}

impl<'tokens, 'src> LabelError<'tokens, Input<'tokens, 'src>, DefaultExpected<'tokens, Token<'src>>>
    for Box<Failure<'src>>
{
    fn expected_found<E: IntoIterator<Item = DefaultExpected<'tokens, Token<'src>>>>(
        expected: E,
        found: Option<MaybeRef<'tokens, Token<'src>>>,
        span: Span,
    ) -> Self {
        Box::new(Failure::new(
            into_found(found),
            expected
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
        ))
    }
}

impl<'tokens, 'src> LabelError<'tokens, Input<'tokens, 'src>, Expected<'src>>
    for Box<Failure<'src>>
{
    fn expected_found<E: IntoIterator<Item = Expected<'src>>>(
        expected: E,
        found: Option<MaybeRef<'tokens, Token<'src>>>,
        span: Span,
    ) -> Self {
        Box::new(Failure::new(
            into_found(found),
            expected.into_iter().collect(),
            span,
        ))
    }

    fn label_with(&mut self, label: Expected<'src>) {
        // note: replaces the label for the expected symbol, retaining the keywords attempted by previous failed parsers
        self.expected = IndexSet::from([label]);
    }
}

/// Convert what chumsky found into a [`Found`].
fn into_found<'tokens, 'src>(found: Option<MaybeRef<'tokens, Token<'src>>>) -> Found<'src> {
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
        let failures = parse!(value(), "@").unwrap_err();

        let messages: Vec<_> = failures
            .into_iter()
            .map(|failure| failure.into_diagnostic().message)
            .collect();

        assert_eq!(
            messages,
            ["expected literal or identifier, found `@`"],
            "merged failures should produce one diagnostic naming every alternative",
        );
    }
}
