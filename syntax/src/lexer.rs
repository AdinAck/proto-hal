//! Transforming the source text into tokens.
//!
//! Lexing is intended to be infallible, as any unrecognized source is put into [`Token::Unrecognized`].

use chumsky::{input::WithContext, prelude::*};

use crate::{
    token::{Doc, Literal, Token, ident, token},
    util::{Span, Spanned},
};

/// The lexer input. The source text with the file's [`SourceId`](crate::util::SourceId) attached.
pub type Input<'src> = WithContext<Span, &'src str>;

/// The lexer's error type, which is infallible.
pub type Error = extra::Err<EmptyErr>;

/// The lexer.
pub fn lexer<'src>() -> impl Parser<'src, Input<'src>, Vec<Spanned<Token<'src>>>, Error> {
    let token = choice((
        doc(),
        literal(),
        ident(),
        operator(),
        punctuation(),
        unrecognized(),
    ))
    .spanned();

    padding().ignore_then(token.then_ignore(padding()).repeated().collect())
}

/// Parse a [`Literal`](crate::token::Literal) from the source. Produces an [unrecognized](Token::Unrecognized) token if
/// the literal is invalid, such as `0b102` or `0x1g`, or overflows.
fn literal<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    // letter, digit, or '_', any of which may participate in a literal
    let participant = any().filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_');

    any()
        .filter(char::is_ascii_digit)
        .then(participant.repeated())
        .to_slice()
        .map(|s: &str| {
            let (digits, radix) = match s.split_at_checked(2) {
                Some(("0x", digits)) => (digits, 16),
                Some(("0b", digits)) => (digits, 2),
                Some(("0o", digits)) => (digits, 8),
                _ => (s, 10),
            };

            u32::from_str_radix(&digits.replace('_', ""), radix)
                .map(|value| Token::Literal(Literal { value, source: s }))
                .unwrap_or(Token::Unrecognized(s))
        })
}

/// An [`Ident`](crate::token::Ident), or a [`Keyword`](crate::token::Keyword).
fn ident<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    text::ident().map(|s: &str| s.parse().map_or(ident![s], Token::Keyword))
}

/// Operators, such as `-`, `.`, `&`, `@`, and `:`.
fn operator<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    choice((
        just("...").to(token![Ellipsis]),
        just("..=").to(token![DotDotEq]),
        just("..").to(token![DotDot]),
        just('.').to(token![Dot]),
        just('|').to(token![Pipe]),
        just('~').to(token![Tilde]),
        just('@').to(token![At]),
        just('&').to(token![Amp]),
        just('-').to(token![Minus]),
        just("::").to(token![::]),
        just(':').to(token![:]),
        just('#').to(token![#]),
    ))
}

/// Punctuation, i.e. `,`, `{`, `}`, `[`, and `]`.
fn punctuation<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    choice((
        just(',').to(token![Comma]),
        just('{').to(token![LBrace]),
        just('}').to(token![RBrace]),
        just('[').to(token![LBracket]),
        just(']').to(token![RBracket]),
    ))
}

/// A [`Doc`](crate::token::Doc) comment, documenting the declaration that follows (`///`) or the file (`//!`).
/// (*See [`comment`]*)
fn doc<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    // after /// or //!
    let text = any()
        .and_is(just('\n').not())
        .repeated()
        .to_slice()
        .map(|s: &str| Doc(s.trim()));

    choice((
        just("///")
            .then(just('/').not())
            .ignore_then(text)
            .map(Token::Doc),
        just("//!").ignore_then(text).map(Token::FileDoc),
    ))
}

/// Inline comments denoted by `//`, or by four or more slashes, such as `////`. (*See [`doc`]*)
fn comment<'src>() -> impl Parser<'src, Input<'src>, (), Error> {
    just("//")
        .and_is(doc().not())
        .then(any().and_is(just('\n').not()).repeated())
        .ignored()
}

/// Whitespace and [comments](comment).
fn padding<'src>() -> impl Parser<'src, Input<'src>, (), Error> {
    choice((text::whitespace().at_least(1), comment())).repeated()
}

/// Capture any remaining input as unrecognized.
fn unrecognized<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error> {
    any().to_slice().map(Token::Unrecognized)
}

#[cfg(test)]
mod tests {
    mod sequence {
        use chumsky::prelude::*;

        use crate::{
            lexer::lexer,
            token::{Doc, Token, ident, keyword, literal, token},
        };

        #[test]
        fn empty() {
            let s = "";

            similar_asserts::assert_eq!(
                expected: Vec::<Token>::new(),
                found: tokens(s),
                "expected lexing an empty source to produce no tokens",
            );
        }

        #[test]
        fn comprehensive() {
            let s = "0xdead_beef // foo\n bar /// baz\n register @ , !";

            let expected = [
                literal![0xdead_beef],
                ident!["bar"],
                Token::Doc(Doc("baz")),
                keyword![Register],
                token![@],
                token![,],
                Token::Unrecognized("!"),
            ];

            similar_asserts::assert_eq!(
                expected: expected.to_vec(),
                found: tokens(s),
                "expected lexing {s:?} to produce a token for each lexeme, excluding comments",
            );
        }

        #[test]
        fn iteration() {
            let s = "foo[a, b] as bar baz[0..=7, ... in #bar] @ [3:0, ... by 4] bam[#bar]";

            let expected = [
                ident!["foo"],
                token![LBracket],
                ident!["a"],
                token![,],
                ident!["b"],
                token![RBracket],
                keyword![As],
                ident!["bar"],
                ident!["baz"],
                token![LBracket],
                literal![0],
                token![..=],
                literal![7],
                token![,],
                token![...],
                keyword![In],
                token![#],
                ident!["bar"],
                token![RBracket],
                token![@],
                token![LBracket],
                literal![3],
                token![:],
                literal![0],
                token![,],
                token![...],
                keyword![By],
                literal![4],
                token![RBracket],
                ident!["bam"],
                token![LBracket],
                token![#],
                ident!["bar"],
                token![RBracket],
            ];

            similar_asserts::assert_eq!(
                expected: expected.to_vec(),
                found: tokens(s),
                "expected lexing {s:?} to produce a token for each lexeme",
            );
        }

        #[test]
        fn no_tokens() {
            let sources = [
                " ",
                "\n",
                "//",
                "// foo",
                "  // foo\n // bar \n",
                "\n// foo\n\n",
                "//// foo\n",
            ];

            for s in sources {
                similar_asserts::assert_eq!(
                    expected: Vec::<Token>::new(),
                    found: tokens(s),
                    "expected lexing {s:?} to produce no tokens",
                );
            }
        }

        /// Lex the provided source, producing the tokens without spans.
        fn tokens(s: &str) -> Vec<Token<'_>> {
            lexer()
                .parse(s.with_context(0))
                .into_result()
                .expect("lexer should be infallible")
                .into_iter()
                .map(|token| *token)
                .collect()
        }
    }

    mod literals {
        use chumsky::prelude::*;

        use crate::{
            lexer::literal,
            token::{Literal, Token},
        };

        #[test]
        fn accept() {
            let literals = [
                ("42", 42),
                ("0xdead__beef", 0xdead_beef),
                ("0x_dead_beef", 0xdead_beef),
                ("42__0_", 420),
                ("0b1010", 0b1010),
            ];

            for (s, value) in literals {
                similar_asserts::assert_eq!(
                    expected: Ok(Token::Literal(Literal { value, source: s })),
                    found: literal().parse(s.with_context(0)).into_result(),
                    "expected `{s}` to be lexed as a literal with value {value}",
                );
            }
        }

        #[test]
        fn display() {
            for s in ["42", "0x2A", "0xdead__beef", "0b10_1010", "0o52"] {
                similar_asserts::assert_eq!(
                    expected: s,
                    found: literal().parse(s.with_context(0)).unwrap().to_string(),
                    "expected display impl for `{s}` to reproduce the source text",
                );
            }
        }

        #[test]
        fn unrecognized() {
            for s in ["0x_", "0b102", "0x1g", "0x", "1foo", "0x1_0000_0000"] {
                similar_asserts::assert_eq!(
                    expected: Ok(Token::Unrecognized(s)),
                    found: literal().parse(s.with_context(0)).into_result(),
                    "expected `{s}` to be lexed as a single unrecognized token",
                );
            }
        }

        #[test]
        fn reject() {
            let not_literals = ["_0b1010", ""];

            for candidate in not_literals {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: literal().parse(candidate.with_context(0)).into_result().ok(),
                    "expected lexing `{candidate}` as a literal to fail",
                );
            }
        }
    }

    mod idents {
        use chumsky::prelude::*;

        use crate::{lexer::ident, token::keyword};

        #[test]
        fn keywords() {
            let idents = [
                ("register", keyword![Register]),
                ("leaky", keyword![Leaky]),
                ("inert", keyword![Inert]),
                ("in", keyword![In]),
                ("refines", keyword![Refines]),
                ("as", keyword![As]),
                ("self", keyword![Self_]),
            ];

            for (s, expected) in idents {
                similar_asserts::assert_eq!(
                    expected: Ok(expected),
                    found: ident().parse(s.with_context(0)).into_result(),
                    "expected `{s}` to be lexed as a keyword",
                );
            }
        }

        #[test]
        fn idents() {
            let idents = ["registe", "_peripheral", "_foo____Bar0", "_", "array"];

            for candidate in idents {
                similar_asserts::assert_eq!(
                    expected: Ok(ident![candidate]),
                    found: ident().parse(candidate.with_context(0)).into_result(),
                    "expected `{candidate}` to be lexed as an identifier",
                );
            }
        }

        #[test]
        fn reject() {
            let not_idents = ["@", "", " ", ",", "1foo", "-"];

            for candidate in not_idents {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: ident().parse(candidate.with_context(0)).into_result().ok(),
                    "expected lexing `{candidate}` as an identifier to fail",
                );
            }
        }
    }

    mod operators {
        use chumsky::prelude::*;

        use crate::lexer::operator;

        #[test]
        fn all() {
            let ops = ["-", "&", "|", "~", "@", "..", "..=", "...", ":", "::", "#"];

            for op in ops {
                operator().parse(op.with_context(0)).unwrap();
            }
        }

        #[test]
        fn reject() {
            let not_ops = [",.", "....", "+", ":::", ""];

            for candidate in not_ops {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: operator().parse(candidate.with_context(0)).into_result().ok(),
                    "expected lexing `{candidate}` as an operator to fail",
                );
            }
        }
    }

    mod puncts {
        use chumsky::prelude::*;

        use crate::lexer::punctuation;

        #[test]
        fn all() {
            let puncts = ",{}[]";

            for c in puncts.chars() {
                let s = c.to_string();
                punctuation().parse(s.with_context(0)).unwrap();
            }
        }

        #[test]
        fn reject() {
            let not_puncts = [
                "#", "@", "$", "%", "a", "A", "0", ":", "/", "(", ")", " ", "",
            ];

            for s in not_puncts {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: punctuation().parse(s.with_context(0)).into_result().ok(),
                    "expected lexing `{s}` as punctuation to fail",
                );
            }
        }
    }

    mod docs {
        use chumsky::prelude::*;

        use crate::{
            lexer::doc,
            token::{Doc, Token},
        };

        #[test]
        fn empty() {
            let input = "";

            similar_asserts::assert_eq!(
                expected: None,
                found: doc().parse(input.with_context(0)).into_result().ok(),
                "expected lexing an empty source as a doc comment to fail",
            );
        }

        #[test]
        fn blank() {
            let input = "///";

            similar_asserts::assert_eq!(
                expected: Token::Doc(Doc("")),
                found: doc().parse(input.with_context(0)).unwrap(),
                "expected `///` alone to be lexed as an empty doc comment",
            );
        }

        #[test]
        fn adjacent() {
            let input = "///foo";

            similar_asserts::assert_eq!(
                expected: Token::Doc(Doc("foo")),
                found: doc().parse(input.with_context(0)).unwrap(),
                "expected the text directly after `///` to be included in the doc comment",
            );
        }

        #[test]
        fn padded() {
            let input = "/// foo \t";

            similar_asserts::assert_eq!(
                expected: Token::Doc(Doc("foo")),
                found: doc().parse(input.with_context(0)).unwrap(),
                "expected the whitespace surrounding the doc comment text to be trimmed",
            );
        }

        #[test]
        fn sentence() {
            let input = "/// foo bar baz buzz!";

            similar_asserts::assert_eq!(
                expected: Token::Doc(Doc("foo bar baz buzz!")),
                found: doc().parse(input.with_context(0)).unwrap(),
                "expected the doc comment text to be lexed as written",
            );
        }

        #[test]
        fn file() {
            let input = "//! foo bar baz buzz!";

            similar_asserts::assert_eq!(
                expected: Token::FileDoc(Doc("foo bar baz buzz!")),
                found: doc().parse(input.with_context(0)).unwrap(),
                "expected `//!` to introduce a file doc comment",
            );
        }

        #[test]
        fn display() {
            for s in ["/// foo", "//! foo", "///"] {
                similar_asserts::assert_eq!(
                    expected: s,
                    found: doc().parse(s.with_context(0)).unwrap().to_string(),
                    "expected display impl for `{s}` to reproduce the source text",
                );
            }
        }

        /// Ensure the [`doc`] parser fails to parse comments.
        #[test]
        fn comment() {
            for input in ["// foo bar baz buzz!", "//// foo", "////"] {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: doc().parse(input.with_context(0)).into_result().ok(),
                    "expected lexing `{input}` as a doc comment to fail",
                );
            }
        }
    }

    mod comments {
        use chumsky::prelude::*;

        use crate::lexer::comment;

        #[test]
        fn empty() {
            let input = "//";

            comment().parse(input.with_context(0)).unwrap();
        }

        #[test]
        fn adjacent() {
            let input = "//foo";

            comment().parse(input.with_context(0)).unwrap();
        }

        #[test]
        fn padded() {
            let input = "// foo \t";

            comment().parse(input.with_context(0)).unwrap();
        }

        #[test]
        fn sentence() {
            let input = "// foo bar baz buzz!";

            comment().parse(input.with_context(0)).unwrap();
        }

        #[test]
        fn confusing_slashes() {
            for input in ["// // foo bar / baz /buzz! //", "//// foo", "////////"] {
                comment().parse(input.with_context(0)).unwrap();
            }
        }

        /// Ensure the [`comment`] parser fails to parse doc comments.
        #[test]
        fn doc() {
            for input in ["/// foo bar baz buzz!", "//! foo bar baz buzz!"] {
                similar_asserts::assert_eq!(
                    expected: None,
                    found: comment().parse(input.with_context(0)).into_result().ok(),
                    "expected lexing `{input}` as a comment to fail",
                );
            }
        }
    }
}
