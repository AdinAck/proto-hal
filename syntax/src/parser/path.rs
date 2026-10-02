//! Parsing of [paths](crate::ast::Path).

use chumsky::prelude::*;

use super::{Error, Input, ident, name::name};
use crate::{
    ast::{DevicePath, ModulePath, Path, PathSegment},
    diagnostic::Expected,
    token::{keyword, token},
};

/// A [`Path`] of segments parsed by `segment`, delineated by their [separator](PathSegment::SEPARATOR).
///
/// ```text
/// path ::= segment (SEPARATOR segment)*
/// ```
fn path<'tokens, 'src: 'tokens, Segment: PathSegment>(
    segment: impl Parser<'tokens, Input<'tokens, 'src>, Segment, Error<'src>> + Clone,
) -> impl Parser<'tokens, Input<'tokens, 'src>, Path<Segment>, Error<'src>> + Clone {
    segment
        .spanned()
        .labelled(Expected::PathSegment)
        .separated_by(just(Segment::SEPARATOR))
        .at_least(1)
        .collect()
        .map(|segments| Path { segments })
}

/// A [`DevicePath`].
///
/// ```text
/// device_path ::= ("device" ".")? name ("." name)*
/// ```
pub(super) fn device_path<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, DevicePath<'src>, Error<'src>> + Clone {
    just(keyword![Device])
        .labelled(Expected::PathSegment)
        .then(just(token![.]))
        .or_not()
        .then(path(name()))
        .map(|(root, path)| DevicePath {
            rooted: root.is_some(),
            path,
        })
}

/// A [`ModulePath`].
///
/// ```text
/// module_path ::= IDENT ("::" IDENT)*
/// ```
pub(super) fn module_path<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, ModulePath<'src>, Error<'src>> + Clone {
    path(ident())
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{
            Failure,
            path::{device_path, module_path},
            tests::parse,
        },
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
                parse!(device_path(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn rooted() {
        assert!(
            parse!(device_path(), "device.foo").unwrap().rooted,
            "`device.` should root the path",
        );
        assert!(
            !parse!(device_path(), "foo").unwrap().rooted,
            "a path without `device.` should not be rooted",
        );
    }

    #[test]
    fn spans() {
        let device_path = parse!(device_path(), "device.foo.bar[a, b]").unwrap();

        let spans: Vec<_> = device_path
            .path
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
                parse!(device_path(), s).unwrap().to_string(),
                expected,
                "whitespace in '{s}' should not be retained"
            );
        }
    }

    #[test]
    fn expected_segment() {
        for (s, expected_found) in [
            ("", Found::End),
            ("@", Found::Token(token![@])),
            ("device.", Found::End),
            ("foo.", Found::End),
            ("foo.0", Found::Token(literal![0])),
            ("foo.#bar", Found::Token(token![#])),
        ] {
            assert_matches!(
                parse!(device_path(), s).unwrap_err().as_slice(),
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
                parse!(device_path(), s).is_err(),
                "'{s}' was parsed as a path when it shouldn't be",
            );
        }
    }

    #[test]
    fn module_paths() {
        for s in ["foo", "foo::bar", "foo::Bar::baz"] {
            assert_eq!(
                parse!(module_path(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn reject_module() {
        for s in [
            "",
            "foo::",
            "foo.bar",
            "device::foo",
            "foo[a, b]",
            "foo::{Bar, Baz}",
            "#foo",
        ] {
            assert!(
                parse!(module_path(), s).is_err(),
                "'{s}' was parsed as a module path when it shouldn't be",
            );
        }
    }
}
