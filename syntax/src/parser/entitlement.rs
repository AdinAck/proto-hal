//! Parsing of [entitlement spaces](crate::ast::EntitlementSpace).

use chumsky::prelude::*;

use super::{Error, Input, path::device_path};
use crate::{
    ast::{EntitlementSpace, Pattern},
    token::token,
};

/// An [`EntitlementSpace`].
///
/// ```text
/// entitlement_space ::= pattern ("|" pattern)*
/// ```
pub(super) fn entitlement_space<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, EntitlementSpace<'src>, Error<'src>> + Clone {
    pattern()
        .spanned()
        .separated_by(just(token![|]))
        .at_least(1)
        .collect()
        .map(|patterns| EntitlementSpace { patterns })
}

/// A [`Pattern`].
///
/// ```text
/// pattern ::= device_path ("&" device_path)*
/// ```
fn pattern<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Pattern<'src>, Error<'src>> + Clone {
    device_path()
        .spanned()
        .separated_by(just(token![&]))
        .at_least(1)
        .collect()
        .map(|paths| Pattern { paths })
}

#[cfg(test)]
mod tests {
    use crate::parser::{entitlement::entitlement_space, tests::parse};

    #[test]
    fn entitlement_spaces() {
        for s in [
            "foo.Bar",
            "foo.{Bar, Baz}",
            "foo.Bar & baz.Qux",
            "foo.Bar | baz.Qux",
            "foo.Bar & baz.Qux | qux.Foo",
            "foo.{Bar, Baz} & baz.Qux",
            "foo.{Bar, Baz} | baz.Qux",
            "foo{1, 2}.bar.Baz",
            "foo{1, 2}.bar.Baz | qux.Foo",
        ] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(entitlement_space(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn precedence() {
        let s = "a.B & c.D | e.F";

        let space = parse!(entitlement_space(), s).unwrap();

        similar_asserts::assert_eq!(
            expected: vec![2, 1],
            found: space
                .patterns
                .iter()
                .map(|pattern| pattern.paths.len())
                .collect::<Vec<_>>(),
            "expected `&` to bind tighter than `|` in `{s}`, producing patterns of two paths and one path",
        );
    }

    #[test]
    fn lines() {
        let s = "foo.Bar & baz.Qux\n    | qux.Foo";

        similar_asserts::assert_eq!(
            expected: "foo.Bar & baz.Qux | qux.Foo",
            found: parse!(entitlement_space(), s).unwrap().to_string(),
            "expected the line break before `|` in {s:?} to be ignored",
        );
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "foo.Bar &",
            "& foo.Bar",
            "foo.Bar |",
            "| foo.Bar",
            "foo.Bar & & baz.Qux",
            "foo.Bar | | baz.Qux",
            "foo.Bar & (baz.Qux | qux.Foo)",
        ] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(entitlement_space(), s).ok().map(|space| space.to_string()),
                "expected parsing `{s}` as an entitlement space to fail",
            );
        }
    }
}
