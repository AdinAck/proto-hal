//! Parsing of [sets](Set).

use chumsky::prelude::*;

use super::{Error, Input, iteration::range, value::value};
use crate::{
    ast::set::{Entry, Set},
    token::token,
};

/// A [`Set`].
///
/// ```text
/// set ::= "{" set_entry ("," set_entry)* ","? "}"
/// ```
pub(super) fn set<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Set<'src>, Error<'src>> + Clone {
    entry()
        .spanned()
        .separated_by(just(token![,]))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(token![LBrace]), just(token![RBrace]))
        .map(|entries| Set { entries })
}

/// A set [`Entry`].
///
/// ```text
/// set_entry ::= range | value
/// ```
fn entry<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Entry<'src>, Error<'src>> + Clone {
    choice((range().map(Entry::Range), value().map(Entry::Value)))
}

#[cfg(test)]
mod tests {
    use crate::parser::{set::set, tests::parse};

    #[test]
    fn sets() {
        for s in ["{A}", "{A, B}", "{1, 3..=5}"] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(set(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn trailing_comma() {
        let s = "{a, b,}";

        similar_asserts::assert_eq!(
            expected: "{a, b}",
            found: parse!(set(), s).unwrap().to_string(),
            "expected display impl for the set parsed from `{s}` to omit the trailing comma",
        );
    }

    #[test]
    fn reject() {
        for s in ["{}", "{0, ...}", "{0", "[0]", "{[0, 1]}", "{{1, 2}}"] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(set(), s).ok().map(|set| set.to_string()),
                "expected parsing `{s}` as a set to fail",
            );
        }
    }
}
