//! Parsing of [sets](crate::ast::Set).

use chumsky::prelude::*;

use super::{Error, Input, entry::entry};
use crate::{
    ast::{Entry, Set},
    span::Spanned,
    token::token,
};

/// A [set](set_of) of [entries](entry).
pub(super) fn set<'src>() -> impl Parser<'src, Input<'src>, Set<'src>, Error<'src>> + Clone {
    set_of(entry())
}

/// A [`Set`].
///
/// ```text
/// set ::= "{" entry ("," entry)* ","? "}"
/// ```
pub(super) fn set_of<'src>(
    entry: impl Parser<'src, Input<'src>, Spanned<Entry<'src>>, Error<'src>> + Clone,
) -> impl Parser<'src, Input<'src>, Set<'src>, Error<'src>> + Clone {
    entry
        .separated_by(just(token![,]))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(token![LBrace]), just(token![RBrace]))
        .map(|entries| Set { entries })
}

#[cfg(test)]
mod tests {
    use crate::parser::{set::set, tests::parse};

    #[test]
    fn sets() {
        for s in ["{A}", "{A, B}", "{1, 3..=5}"] {
            assert_eq!(
                parse(set(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn trailing_comma() {
        assert_eq!(
            parse(set(), "{a, b,}").unwrap().to_string(),
            "{a, b}",
            "trailing comma should not be retained",
        );
    }

    #[test]
    fn reject() {
        for s in ["{}", "{0, ...}", "{0", "[0]"] {
            assert!(
                parse(set(), s).is_err(),
                "'{s}' was parsed as a set when it shouldn't be"
            );
        }
    }
}
