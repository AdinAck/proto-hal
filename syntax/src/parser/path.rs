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
    use crate::{
        diagnostic::{Expected, Found},
        parser::{
            path::{device_path, module_path},
            tests::{expectations, parse},
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
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(device_path(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn rooted() {
        similar_asserts::assert_eq!(
            expected: true,
            found: parse!(device_path(), "device.foo").unwrap().rooted,
            "expected the device path parsed from `device.foo` to be rooted by `device.`",
        );
        similar_asserts::assert_eq!(
            expected: false,
            found: parse!(device_path(), "foo").unwrap().rooted,
            "expected the device path parsed from `foo` to be unrooted, lacking `device.`",
        );
    }

    #[test]
    fn spans() {
        let s = "device.foo.bar[a, b]";

        let device_path = parse!(device_path(), s).unwrap();

        similar_asserts::assert_eq!(
            expected: vec![7..10, 11..20],
            found: device_path
                .path
                .segments
                .iter()
                .map(|segment| segment.span.range())
                .collect::<Vec<_>>(),
            "expected the spans produced by parsing `{s}` to correspond to the path segments",
        );
    }

    #[test]
    fn whitespace() {
        for (s, expected) in [
            ("foo .bar", "foo.bar"),
            ("foo. bar", "foo.bar"),
            ("device . foo", "device.foo"),
        ] {
            similar_asserts::assert_eq!(
                expected: expected,
                found: parse!(device_path(), s).unwrap().to_string(),
                "expected display impl for the device path parsed from `{s}` to omit the whitespace",
            );
        }
    }

    #[test]
    fn expected_segment() {
        for (s, found) in [
            ("", Found::End),
            ("@", Found::Token(token![@])),
            ("device.", Found::End),
            ("foo.", Found::End),
            ("foo.0", Found::Token(literal![0])),
            ("foo.#bar", Found::Token(token![#])),
        ] {
            similar_asserts::assert_eq!(
                expected: vec![(found, vec![Expected::PathSegment])],
                found: expectations(parse!(device_path(), s).unwrap_err()),
                "expected parsing `{s}` as a device path to fail, expecting a path segment",
            );
        }
    }

    #[test]
    fn reject() {
        for s in ["", "device", "device.", ".foo", "foo..bar", "foo.device"] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(device_path(), s).ok().map(|path| path.to_string()),
                "expected parsing `{s}` as a device path to fail",
            );
        }
    }

    #[test]
    fn module_paths() {
        for s in ["foo", "foo::bar", "foo::Bar::baz"] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(module_path(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
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
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(module_path(), s).ok().map(|path| path.to_string()),
                "expected parsing `{s}` as a module path to fail",
            );
        }
    }
}
