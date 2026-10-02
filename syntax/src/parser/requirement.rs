//! Parsing of [requirements](crate::ast::Requirement).

use chumsky::prelude::*;

use super::{Error, Input, path::path};
use crate::{
    ast::{Pattern, Requirement},
    token::token,
};

/// A [`Requirement`].
///
/// ```text
/// requirement ::= pattern ("|" pattern)*
/// ```
#[cfg_attr(not(test), expect(unused, reason = "for future use"))]
pub(super) fn requirement<'src>()
-> impl Parser<'src, Input<'src>, Requirement<'src>, Error<'src>> + Clone {
    pattern()
        .spanned()
        .separated_by(just(token![|]))
        .at_least(1)
        .collect()
        .map(|patterns| Requirement { patterns })
}

/// A [`Pattern`].
///
/// ```text
/// pattern ::= path ("&" path)*
/// ```
fn pattern<'src>() -> impl Parser<'src, Input<'src>, Pattern<'src>, Error<'src>> + Clone {
    path()
        .spanned()
        .separated_by(just(token![&]))
        .at_least(1)
        .collect()
        .map(|paths| Pattern { paths })
}

#[cfg(test)]
mod tests {
    use crate::parser::{requirement::requirement, tests::parse};

    #[test]
    fn requirements() {
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
                parse(requirement(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn precedence() {
        let requirement = parse(requirement(), "a.B & c.D | e.F").unwrap();

        let shape: Vec<_> = requirement
            .patterns
            .iter()
            .map(|pattern| pattern.paths.len())
            .collect();

        assert_eq!(shape, [2, 1], "`&` should bind tighter than `|`");
    }

    #[test]
    fn lines() {
        assert_eq!(
            parse(requirement(), "foo.Bar & baz.Qux\n    | qux.Foo")
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
                parse(requirement(), s).is_err(),
                "'{s}' was parsed as a requirement when it shouldn't be",
            );
        }
    }
}
