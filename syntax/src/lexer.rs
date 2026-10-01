//! Transforming the source text into tokens.
//!
//! Lexing is intended to be infallible, as any unrecognized source is put into [`Token::Unrecognized`].

use chumsky::{input::WithContext, prelude::*};

use crate::{
    span::{Span, Spanned},
    token::{Doc, Token, ident, keyword, literal, token},
};

/// The lexer input. The source text with the file's [`SourceId`](crate::span::SourceId) attached.
pub type Input<'src> = WithContext<Span, &'src str>;

/// The error type emitted by the lexer.
pub type Error<'src> = extra::Err<Rich<'src, char, Span>>;

/// The lexer.
pub fn lexer<'src>() -> impl Parser<'src, Input<'src>, Vec<Spanned<Token<'src>>>, Error<'src>> {
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

/// A [`Literal`](crate::token::Literal).
fn literal<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
    // digit sequence with '_' separators
    let digits = |radix| {
        any()
            .filter(move |c: &char| c.is_digit(radix))
            .separated_by(just('_').repeated())
            .at_least(1)
    };

    choice((
        just("0x").then(digits(16)).ignored(),
        just("0b").then(digits(2)).ignored(),
        just("0o").then(digits(8)).ignored(),
        digits(10).ignored(),
    ))
    .to_slice()
    .map(|s: &str| {
        let (digits, radix) = match s.split_at_checked(2) {
            Some(("0x", digits)) => (digits, 16),
            Some(("0b", digits)) => (digits, 2),
            Some(("0o", digits)) => (digits, 8),
            _ => (s, 10),
        };

        u32::from_str_radix(&digits.replace('_', ""), radix)
            .map(|value| literal![value])
            .unwrap_or(Token::Unrecognized(s))
    })
    .labelled("integer literal")
}

/// An [`Ident`](crate::token::Ident), or a [`Keyword`](crate::token::Keyword).
fn ident<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
    text::ident().map(|s: &str| match s {
        "device" => keyword![Device],
        "import" => keyword![Import],
        "peripheral" => keyword![Peripheral],
        "register" => keyword![Register],
        "field" => keyword![Field],
        "schema" => keyword![Schema],
        "variant" => keyword![Variant],
        "group" => keyword![Group],
        "interrupts" => keyword![Interrupts],
        "requires" => keyword![Requires],
        "read" => keyword![Read],
        "write" => keyword![Write],
        "store" => keyword![Store],
        "volatile" => keyword![Volatile],
        "hardware" => keyword![Hardware],
        "leaky" => keyword![Leaky],
        "inert" => keyword![Inert],
        "as" => keyword![As],
        "by" => keyword![By],
        "in" => keyword![In],
        "refines" => keyword![Refines],
        "assumes" => keyword![Assumes],
        "reset" => keyword![Reset],
        _ => ident![s],
    })
}

/// Operators i.e. `-`, `.`, `&`, `@`, `:`, etc.
fn operator<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
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
        just(':').to(token![:]),
        just('#').to(token![#]),
    ))
    .labelled("operator")
}

/// Punctuation i.e. `,`, `{`, `}`, `[`, `]`.
fn punctuation<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
    choice((
        just(',').to(token![Comma]),
        just('{').to(token![LBrace]),
        just('}').to(token![RBrace]),
        just('[').to(token![LBracket]),
        just(']').to(token![RBracket]),
    ))
    .labelled("punctuation")
}

/// A [`Doc`](crate::token::Doc) comment. See [`comment`] for inline comments (`//`).
fn doc<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
    just("///")
        .ignore_then(any().and_is(just('\n').not()).repeated().to_slice())
        .map(|s: &str| Token::Doc(Doc(s.trim())))
        .labelled("doc comment")
}

/// Inline comments denoted by `//`. See [`doc`] for doc comments (`///`).
fn comment<'src>() -> impl Parser<'src, Input<'src>, (), Error<'src>> {
    just("//")
        .then(just('/').not())
        .then(any().and_is(just('\n').not()).repeated())
        .labelled("comment")
        .padded()
        .ignored()
}

/// Whitespace and [comments](comment): everything between tokens.
fn padding<'src>() -> impl Parser<'src, Input<'src>, (), Error<'src>> {
    comment().repeated().padded()
}

/// Capture any remaining input as unrecognized.
fn unrecognized<'src>() -> impl Parser<'src, Input<'src>, Token<'src>, Error<'src>> {
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

            assert!(
                lexer()
                    .parse(s.with_context(0))
                    .into_result()
                    .expect("lexer should be infallible")
                    .is_empty(),
                "expected empty string to produce zero tokens",
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

            itertools::assert_equal(
                lexer()
                    .parse(s.with_context(0))
                    .into_result()
                    .expect("lexer should be infallible")
                    .into_iter()
                    .map(|token| *token),
                expected,
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

            itertools::assert_equal(
                lexer()
                    .parse(s.with_context(0))
                    .into_result()
                    .expect("lexer should be infallible")
                    .into_iter()
                    .map(|token| *token),
                expected,
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
            ];

            for s in sources {
                assert!(
                    lexer()
                        .parse(s.with_context(0))
                        .into_result()
                        .expect("lexer should be infallible")
                        .is_empty(),
                    "expected {s:?} to produce zero tokens",
                );
            }
        }
    }

    mod literals {
        use chumsky::prelude::*;

        use crate::lexer::literal;

        #[test]
        fn accept() {
            let literals = [
                ("42", 42),
                ("0xdead__beef", 0xdead_beef),
                ("0b1010", 0b1010),
            ];

            for (s, expected) in literals {
                assert!(
                    literal()
                        .parse(s.with_context(0))
                        .into_result()
                        .is_ok_and(|out| out == literal![expected]),
                    "'{s}' was not parsed as expected literal '{expected:?}'",
                );
            }
        }

        #[test]
        fn reject() {
            let not_literals = ["42__0_", "0x_dead_beef", "_0b1010", ""];

            for candidate in not_literals {
                assert!(
                    literal()
                        .parse(candidate.with_context(0))
                        .into_result()
                        .is_err(),
                    "'{candidate}' was parsed as a literal when it shouldn't be",
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
            ];

            for (s, expected) in idents {
                assert!(
                    ident()
                        .parse(s.with_context(0))
                        .into_result()
                        .is_ok_and(|out| out == expected),
                    "'{s}' was not parsed as expected keyword '{expected:?}'",
                );
            }
        }

        #[test]
        fn idents() {
            let idents = ["registe", "_peripheral", "_foo____Bar0", "_", "array"];

            for candidate in idents {
                assert!(
                    ident()
                        .parse(candidate.with_context(0))
                        .into_result()
                        .is_ok_and(|out| out == ident![candidate]),
                    "'{candidate}' was not parsed as an identifier when it should be",
                );
            }
        }

        #[test]
        fn reject() {
            let not_idents = ["@", "", " ", ",", "1foo", "-"];

            for candidate in not_idents {
                assert!(
                    ident()
                        .parse(candidate.with_context(0))
                        .into_result()
                        .is_err(),
                    "'{candidate}' was parsed as an identifier when it shouldn't be",
                );
            }
        }
    }

    mod operators {
        use chumsky::prelude::*;

        use crate::lexer::operator;

        #[test]
        fn all() {
            let ops = ["-", "&", "|", "~", "@", "..", "..=", "...", ":", "#"];

            for op in ops {
                operator().parse(op.with_context(0)).unwrap();
            }
        }

        #[test]
        fn reject() {
            let not_ops = [",.", "....", "+", "::", ""];

            for candidate in not_ops {
                assert!(
                    operator()
                        .parse(candidate.with_context(0))
                        .into_result()
                        .is_err(),
                    "'{candidate}' was parsed as an operator when it shouldn't be",
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
                assert!(
                    punctuation()
                        .parse(s.with_context(0))
                        .into_result()
                        .is_err(),
                    "'{s}' was parsed as punctuation when it shouldn't be",
                );
            }
        }
    }

    mod docs {
        use std::assert_matches;

        use chumsky::prelude::*;

        use crate::{
            lexer::doc,
            token::{Doc, Token},
        };

        #[test]
        fn empty() {
            let input = "";

            assert!(
                doc().parse(input.with_context(0)).into_result().is_err(),
                "an empty source should not be a doc comment",
            );
        }

        #[test]
        fn blank() {
            let input = "///";

            let lexed = doc().parse(input.with_context(0)).unwrap();

            assert_matches!(
                lexed,
                Token::Doc(Doc("")),
                "`///` alone should be an empty doc comment",
            );
        }

        #[test]
        fn adjacent() {
            let input = "///foo";

            let lexed = doc().parse(input.with_context(0)).unwrap();

            assert_matches!(
                lexed,
                Token::Doc(Doc("foo")),
                "text directly after `///` should be included",
            );
        }

        #[test]
        fn padded() {
            let input = "/// foo \t";

            let lexed = doc().parse(input.with_context(0)).unwrap();

            assert_matches!(
                lexed,
                Token::Doc(Doc("foo")),
                "surrounding whitespace should be trimmed",
            );
        }

        #[test]
        fn sentence() {
            let input = "/// foo bar baz buzz!";

            let lexed = doc().parse(input.with_context(0)).unwrap();

            assert_matches!(lexed, Token::Doc(Doc("foo bar baz buzz!")));
        }

        /// Ensure the [`doc`] parser fails to parse comments.
        #[test]
        fn comment() {
            let input = "// foo bar baz buzz!";

            assert!(doc().parse(input.with_context(0)).into_result().is_err());
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
            let input = "// // foo bar / baz /buzz! //";

            comment().parse(input.with_context(0)).unwrap();
        }

        /// Ensure the [`comment`] parser fails to parse doc comments.
        #[test]
        fn doc() {
            let input = "/// foo bar baz buzz!";

            assert!(
                comment()
                    .parse(input.with_context(0))
                    .into_result()
                    .is_err()
            );
        }
    }
}
