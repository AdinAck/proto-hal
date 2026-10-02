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
            assert_eq!(
                parse!(entitlement_space(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn precedence() {
        let space = parse!(entitlement_space(), "a.B & c.D | e.F").unwrap();

        let shape: Vec<_> = space
            .patterns
            .iter()
            .map(|pattern| pattern.paths.len())
            .collect();

        assert_eq!(shape, [2, 1], "`&` should bind tighter than `|`");
    }

    #[test]
    fn lines() {
        assert_eq!(
            parse!(entitlement_space(), "foo.Bar & baz.Qux\n    | qux.Foo")
                .unwrap()
                .to_string(),
            "foo.Bar & baz.Qux | qux.Foo",
            "a line break before `|` should not matter",
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
            assert!(
                parse!(entitlement_space(), s).is_err(),
                "'{s}' was parsed as an entitlement space when it shouldn't be",
            );
        }
    }
}
