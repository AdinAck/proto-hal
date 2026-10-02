//! Parsing of [paths](crate::ast::Path).

use chumsky::prelude::*;

use super::{Error, Input, name::name};
use crate::{
    ast::Path,
    diagnostic::Expected,
    token::{keyword, token},
};

/// A [`Path`].
///
/// ```text
/// path ::= ("device" ".")? name ("." name)*
/// ```
pub(super) fn path<'src>() -> impl Parser<'src, Input<'src>, Path<'src>, Error<'src>> + Clone {
    let segment = just(token![.]).ignore_then(name().spanned().labelled(Expected::PathSegment));

    choice((
        just(keyword![Device])
            .ignore_then(segment.clone())
            .map(|first| (true, first)),
        name().spanned().map(|first| (false, first)),
    ))
    .then(segment.repeated().collect::<Vec<_>>())
    .map(|((rooted, first), rest)| Path {
        rooted,
        segments: [first].into_iter().chain(rest).collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{Failure, path::path, tests::parse},
        token::{literal, token},
    };

    #[test]
    fn paths() {
        for s in [
            "foo",
            "foo.bar",
            "device.foo.Bar",
            "foo[#bar]baz.Qux",
            "foo{1, 2}.bar.{Baz, Qux}",
            "[foo, bar].baz",
        ] {
            assert_eq!(
                parse(path(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn rooted() {
        assert!(
            parse(path(), "device.foo").unwrap().rooted,
            "`device.` should root the path",
        );
        assert!(
            !parse(path(), "foo").unwrap().rooted,
            "a path without `device.` should not be rooted",
        );
    }

    #[test]
    fn spans() {
        let path = parse(path(), "device.foo.bar[a, b]").unwrap();

        let spans: Vec<_> = path
            .segments
            .iter()
            .map(|segment| segment.span.range())
            .collect();

        assert_eq!(
            spans,
            [7..10, 11..20],
            "produced spans were not as expected"
        );
    }

    #[test]
    fn whitespace() {
        for (s, expected) in [
            ("foo .bar", "foo.bar"),
            ("foo. bar", "foo.bar"),
            ("device . foo", "device.foo"),
        ] {
            assert_eq!(
                parse(path(), s).unwrap().to_string(),
                expected,
                "whitespace in '{s}' should not be retained"
            );
        }
    }

    #[test]
    fn expected_segment() {
        for (s, expected_found) in [
            ("foo.", Found::End),
            ("foo.0", Found::Token(literal![0])),
            ("foo.#bar", Found::Token(token![#])),
        ] {
            assert_matches!(
                parse(path(), s).unwrap_err().as_slice(),
                [Failure { found, expected, .. }]
                    if *found == expected_found && expected.iter().eq(&[Expected::PathSegment]),
                "'{s}' should expect a path segment",
            );
        }
    }

    #[test]
    fn reject() {
        for s in ["", "device", "device.", ".foo", "foo..bar", "foo.device"] {
            assert!(
                parse(path(), s).is_err(),
                "'{s}' was parsed as a path when it shouldn't be",
            );
        }
    }
}
