//! Parsing of [lists](List), [ranges](Range), [progressions](Progression), and iteration variables.

use chumsky::prelude::*;

use super::{Error, Input, entry::entry, ident, literal};
use crate::{
    ast::{Entry, List, Progression, Range},
    diagnostic::Expected,
    span::Spanned,
    token::{Ident, keyword, token},
};

/// A [list](list_of) of [entries](entry).
pub(super) fn list<'src>() -> impl Parser<'src, Input<'src>, List<'src>, Error<'src>> + Clone {
    list_of(entry())
}

/// A [`List`].
///
/// ```text
/// list ::= "[" entry ("," entry)* ("," progression)? ","? "]"
/// ```
pub(super) fn list_of<'src>(
    entry: impl Parser<'src, Input<'src>, Spanned<Entry<'src>>, Error<'src>> + Clone,
) -> impl Parser<'src, Input<'src>, List<'src>, Error<'src>> + Clone {
    let comma = just(token![,]);

    entry
        .separated_by(comma)
        .at_least(1)
        .collect()
        .then(comma.ignore_then(progression().spanned()).or_not())
        .then_ignore(comma.or_not())
        .delimited_by(just(token![LBracket]), just(token![RBracket]))
        .map(|(entries, progression)| List {
            entries,
            progression,
        })
}

/// A [`Range`].
///
/// ```text
/// range ::= LITERAL (".." | "..=") LITERAL step?
/// ```
pub(super) fn range<'src>() -> impl Parser<'src, Input<'src>, Range, Error<'src>> + Clone {
    literal()
        .then(choice((
            just(token![..]).to(false),
            just(token![..=]).to(true),
        )))
        .then(literal())
        .then(step().or_not())
        .map(|(((start, inclusive), end), step)| Range {
            start,
            end,
            inclusive,
            step,
        })
}

/// A [`Progression`].
///
/// ```text
/// progression ::= "..." step? ("in" variable)?
/// ```
fn progression<'src>() -> impl Parser<'src, Input<'src>, Progression<'src>, Error<'src>> + Clone {
    just(token![...])
        .ignore_then(step().or_not())
        .then(
            just(keyword![In])
                .ignore_then(variable().spanned())
                .or_not(),
        )
        .map(|(step, scope)| Progression { step, scope })
}

/// The distance between successive values or sequences i.e. `by 0x14`, `by -4`.
///
/// ```text
/// step ::= "by" "-"? LITERAL
/// ```
fn step<'src>() -> impl Parser<'src, Input<'src>, i64, Error<'src>> + Clone {
    just(keyword![By])
        .ignore_then(just(token![-]).or_not())
        .then(literal())
        .map(|(minus, n)| match minus {
            Some(..) => -i64::from(*n),
            None => i64::from(*n),
        })
}

/// An iteration variable i.e. `#foo`.
///
/// ```text
/// variable ::= "#" IDENT
/// ```
pub(super) fn variable<'src>() -> impl Parser<'src, Input<'src>, Ident<'src>, Error<'src>> + Clone {
    just(token![#]).ignore_then(ident().labelled(Expected::IterationVariable))
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{Failure, iteration::list, tests::parse},
        token::{literal, token},
    };

    #[test]
    fn lists() {
        for s in [
            "[0]",
            "[a, b, c]",
            "[3:0, 7:4]",
            "[0..16, 0..=15 by 4]",
            "[[0, 2..=7], ...]",
            "[{1, 2}, {3..=5}]",
            "[8, ...]",
            "[1:0, ... by -4]",
            "[0..=7, ... in #foo]",
            "[0..=5, ... by 16 in #bar]",
        ] {
            assert_eq!(
                parse(list(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn trailing_comma() {
        for (parsed, expected) in [
            (parse(list(), "[a, b,]").unwrap().to_string(), "[a, b]"),
            (parse(list(), "[a, ...,]").unwrap().to_string(), "[a, ...]"),
        ] {
            assert_eq!(parsed, expected, "trailing comma should not be retained");
        }
    }

    #[test]
    fn spans() {
        let list = parse(list(), "[foo, 0..=7]").unwrap();

        let spans: Vec<_> = list
            .entries
            .iter()
            .map(|entry| entry.span.range())
            .collect();

        assert_eq!(spans, [1..4, 6..11], "produced spans were not as expected");
    }

    #[test]
    fn reject_lists() {
        for s in [
            "[]",
            "[...]",
            "[0, ..., 1]",
            "[0, ..., ...]",
            "[0,, 1]",
            "[, 0]",
            "[0 1]",
            "[0..]",
            "[..=7]",
            "[0 by 4]",
            "[0, ... in foo]",
            "[0, ... in #foo by 4]",
            "[0, by -4]",
            "[#]",
            "[#foo]",
            "[0",
        ] {
            assert!(
                parse(list(), s).is_err(),
                "'{s}' was parsed as a list when it shouldn't be"
            );
        }
    }

    #[test]
    fn variable_whitespace() {
        assert_eq!(
            parse(list(), "[0, ... in # foo]").unwrap().to_string(),
            "[0, ... in #foo]",
            "whitespace after `#` should not be retained"
        );
    }

    #[test]
    fn variable_expected() {
        for (s, expected_found) in [
            ("[0, ... in #0]", Found::Token(literal![0])),
            ("[0, ... in #]", Found::Token(token![RBracket])),
        ] {
            assert_matches!(
                parse(list(), s).unwrap_err().as_slice(),
                [Failure { found, expected, .. }]
                    if *found == expected_found && expected.iter().eq(&[Expected::IterationVariable]),
                "'{s}' should expect an iteration variable",
            );
        }
    }
}
