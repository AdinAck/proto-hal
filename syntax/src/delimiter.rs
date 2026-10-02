//! Balancing of delimiters, between [lexing](crate::lexer) and [parsing](crate::parser).

use indexmap::IndexSet;

use crate::{
    diagnostic::{Diagnostic, Found, unclosed, unexpected},
    token::{LBrace, LBracket, Punctuation, RBrace, RBracket, Token},
    util::{Span, Spanned},
};

/// Balance the delimiters of `tokens`, so that every opening delimiter has a closing delimiter. A missing closing
/// delimiter is inserted into the produced tokens where it was expected, before an outer closing delimiter or at
/// `end`. A closing delimiter without an opening delimiter is omitted from the produced tokens.
pub fn balance<'src>(
    tokens: Vec<Spanned<Token<'src>>>,
    end: Span,
) -> (Vec<Spanned<Token<'src>>>, Vec<Diagnostic>) {
    // opening delimiters awaiting their closing delimiter, with their span, innermost last
    let mut open = Vec::new();
    let mut balanced = Vec::with_capacity(tokens.len());
    let mut diagnostics = Vec::new();

    for token in tokens {
        match *token {
            Token::Punctuation(LBrace) => open.push((LBrace, RBrace, token.span)),
            Token::Punctuation(LBracket) => open.push((LBracket, RBracket, token.span)),
            Token::Punctuation(closer @ (RBrace | RBracket)) => {
                let Some(index) = open
                    .iter()
                    .rposition(|(_, expected, _)| *expected == closer)
                else {
                    diagnostics.push(unexpected(
                        token.span,
                        Found::Token(*token),
                        &IndexSet::new(),
                        &IndexSet::new(),
                    ));
                    continue;
                };

                for (closer, diagnostic) in open
                    .drain(index + 1..)
                    .rev()
                    .map(|open| close(open, token.span))
                {
                    balanced.push(closer);
                    diagnostics.push(diagnostic);
                }

                open.pop();
            }
            _ => {}
        }

        balanced.push(token);
    }

    for (closer, diagnostic) in open.into_iter().rev().map(|open| close(open, end)) {
        balanced.push(closer);
        diagnostics.push(diagnostic);
    }

    (balanced, diagnostics)
}

/// Produce the missing closing delimiter expected at `at`, with an empty span at its start, along with its diagnostic.
fn close<'src>(
    (opener, closer, opening): (Punctuation, Punctuation, Span),
    at: Span,
) -> (Spanned<Token<'src>>, Diagnostic) {
    (
        Spanned {
            inner: Token::Punctuation(closer),
            span: Span {
                end: at.start,
                ..at
            },
        },
        unclosed(
            Spanned {
                inner: opener,
                span: opening,
            },
            Spanned {
                inner: closer,
                span: at,
            },
        ),
    )
}

#[cfg(test)]
mod tests {
    use chumsky::{input::Input as _, prelude::*};
    use itertools::Itertools;

    use crate::{
        delimiter::balance,
        diagnostic::{Diagnostic, Kind},
        lexer::lexer,
        token::Token,
        util::{Span, Spanned},
    };

    /// The end of `s`.
    fn end(s: &str) -> Span {
        Span {
            source: 0,
            start: s.len(),
            end: s.len(),
        }
    }

    /// Lex `s` and balance its delimiters.
    fn lex(s: &'static str) -> (Vec<Spanned<Token<'static>>>, Vec<Diagnostic>) {
        balance(
            lexer().parse(s.with_context(0)).into_result().unwrap(),
            end(s),
        )
    }

    #[test]
    fn balanced() {
        for s in ["", "foo", "{ foo [ bar ] { } }", "[ [ ] ] { }"] {
            let (tokens, diagnostics) = lex(s);

            assert_eq!(tokens.iter().join(" "), s, "'{s}' should be unchanged");
            assert!(
                diagnostics.is_empty(),
                "'{s}' should produce no diagnostics"
            );
        }
    }

    #[test]
    fn unbalanced() {
        for (s, expected, expected_diagnostics) in [
            (
                "foo { bar",
                "foo { bar }",
                [(Kind::Unclosed, "unclosed `{`")].as_slice(),
            ),
            (
                "foo [ bar",
                "foo [ bar ]",
                [(Kind::Unclosed, "unclosed `[`")].as_slice(),
            ),
            (
                "{ [",
                "{ [ ] }",
                [
                    (Kind::Unclosed, "unclosed `[`"),
                    (Kind::Unclosed, "unclosed `{`"),
                ]
                .as_slice(),
            ),
            (
                "{ foo [ bar }",
                "{ foo [ bar ] }",
                [(Kind::Unclosed, "unclosed `[`")].as_slice(),
            ),
            (
                "foo } bar",
                "foo bar",
                [(Kind::Unexpected, "unexpected `}`")].as_slice(),
            ),
            (
                "[ } ]",
                "[ ]",
                [(Kind::Unexpected, "unexpected `}`")].as_slice(),
            ),
        ] {
            let (tokens, diagnostics) = lex(s);

            assert_eq!(
                tokens.iter().join(" "),
                expected,
                "'{s}' should be balanced as '{expected}'"
            );
            assert_eq!(
                diagnostics
                    .iter()
                    .map(|diagnostic| &diagnostic.message)
                    .collect_vec(),
                expected_diagnostics
                    .iter()
                    .map(|(.., message)| message)
                    .collect_vec(),
                "'{s}' should produce a diagnostic for each delimiter balanced"
            );
            assert!(
                diagnostics
                    .iter()
                    .zip(expected_diagnostics)
                    .all(|(diagnostic, (kind, ..))| diagnostic.is(*kind)),
                "'{s}' should produce diagnostics of the expected kinds"
            );
        }
    }

    #[test]
    fn spans() {
        for (s, token_spans, label_spans) in [
            ("{", [0..1, 1..1].as_slice(), (1..1, 0..1)),
            ("{ [ }", [0..1, 2..3, 4..4, 4..5].as_slice(), (4..5, 2..3)),
        ] {
            let (tokens, diagnostics) = lex(s);

            assert_eq!(
                tokens.iter().map(|token| token.span.range()).collect_vec(),
                token_spans,
                "an inserted closing delimiter should have an empty span where it was expected",
            );
            assert_eq!(
                diagnostics
                    .iter()
                    .map(|diagnostic| (
                        diagnostic.primary_label.span.range(),
                        diagnostic.supporting_labels[0].span.range()
                    ))
                    .collect_vec(),
                [label_spans],
                "the diagnostic should label where the closing delimiter was expected, and the opening delimiter",
            );
        }
    }
}
