//! Parsing of [properties](Property).

use chumsky::prelude::*;

use super::{
    Error, Input, entitlement::entitlement_space, list::list, path::module_path, set::set,
    value::value,
};
use crate::{
    ast::{Argument, Property, RequirementQualifiers},
    diagnostic::Expected,
    token::{keyword, token},
};

/// A [`Property`].
///
/// ```text
/// property ::= "@" argument | "~" argument | "reset" argument | "wordsize" argument
///            | "access" set
///            | "assumes" module_path
///            | "refines" module_path ("," module_path)*
///            | ("hardware"? "write")? "requires" entitlement_space
/// ```
pub(super) fn property<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Property<'src>, Error<'src>> + Clone {
    // after @, ~, reset, or wordsize
    let argument = argument().spanned().labelled(Expected::Value);
    // after assumes or refines
    let definition = module_path().spanned().labelled(Expected::ModulePath);
    // before requires
    let qualifiers = choice((
        just(keyword![Hardware])
            .then(just(keyword![Write]))
            .to(RequirementQualifiers::HardwareWrite),
        just(keyword![Write]).to(RequirementQualifiers::Write),
    ));

    choice((
        just(token![@])
            .ignore_then(argument.clone())
            .map(Property::Placement),
        just(token![~])
            .ignore_then(argument.clone())
            .map(Property::Discriminant),
        just(keyword![Reset])
            .ignore_then(argument.clone())
            .map(Property::Reset),
        just(keyword![WordSize])
            .ignore_then(argument)
            .map(Property::WordSize),
        just(keyword![Access])
            .ignore_then(set().spanned().labelled(Expected::Set))
            .map(Property::Access),
        just(keyword![Assumes])
            .ignore_then(definition.clone())
            .map(Property::Assumes),
        just(keyword![Refines])
            .ignore_then(
                definition
                    .separated_by(just(token![,]))
                    .at_least(1)
                    .collect(),
            )
            .map(Property::Refines),
        qualifiers
            .or_not()
            .then_ignore(just(keyword![Requires]))
            .then(
                entitlement_space()
                    .spanned()
                    .labelled(Expected::EntitlementSpace),
            )
            .map(|(qualifiers, space)| Property::Requires { qualifiers, space }),
    ))
}

/// An [`Argument`].
///
/// ```text
/// argument ::= value | list
/// ```
fn argument<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Argument<'src>, Error<'src>> + Clone {
    choice((value().map(Argument::Value), list().map(Argument::List)))
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{Property, RequirementQualifiers},
        diagnostic::{Expected, Found},
        parser::{
            property::property,
            tests::{expectations, parse},
        },
        token::{keyword, literal, token},
    };

    #[test]
    fn properties() {
        for s in [
            "@ 0x14",
            "@ 15:0",
            "@ [1:0, ...]",
            "~ 0",
            "~ [0, 1]",
            "reset 0",
            "reset foo",
            "wordsize 4",
            "access {4}",
            "access {1, 2, 4}",
            "assumes foo",
            "assumes foo::bar",
            "refines foo",
            "refines foo, bar::baz",
            "requires foo.Bar",
            "write requires foo.Bar & baz.Qux",
            "hardware write requires foo.{Bar, Baz} | qux.Foo",
        ] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(property(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn qualifiers() {
        for (s, expected) in [
            ("requires foo.Bar", None),
            ("write requires foo.Bar", Some(RequirementQualifiers::Write)),
            (
                "hardware write requires foo.Bar",
                Some(RequirementQualifiers::HardwareWrite),
            ),
        ] {
            let Property::Requires { qualifiers, .. } = parse!(property(), s).unwrap() else {
                panic!("expected `{s}` to be parsed as a `requires` property");
            };

            similar_asserts::assert_eq!(
                expected: expected,
                found: qualifiers,
                "expected the requirement qualifiers parsed from `{s}` to be {expected:?}",
            );
        }
    }

    #[test]
    fn expected() {
        for (s, found, expected) in [
            ("@", Found::End, Expected::Value),
            ("~ ,", Found::Token(token![,]), Expected::Value),
            ("reset @", Found::Token(token![@]), Expected::Value),
            ("wordsize", Found::End, Expected::Value),
            ("access 4", Found::Token(literal![4]), Expected::Set),
            ("assumes", Found::End, Expected::ModulePath),
            ("refines foo,", Found::End, Expected::ModulePath),
            ("assumes foo::", Found::End, Expected::PathSegment),
            (
                "requires @",
                Found::Token(token![@]),
                Expected::EntitlementSpace,
            ),
            (
                "hardware requires",
                Found::Token(keyword![Requires]),
                Expected::Token(keyword![Write]),
            ),
        ] {
            similar_asserts::assert_eq!(
                expected: vec![(found, vec![expected])],
                found: expectations(parse!(property(), s).unwrap_err()),
                "expected parsing `{s}` as a property to fail, expecting {expected}",
            );
        }
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "@ foo.bar",
            "reset",
            "wordsize {4}",
            "access",
            "access {}",
            "assumes foo, bar",
            "assumes foo.bar",
            "refines",
            "refines foo,, bar",
            "write",
            "write hardware requires foo.Bar",
            "requires",
        ] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(property(), s).ok().map(|property| property.to_string()),
                "expected parsing `{s}` as a property to fail",
            );
        }
    }
}
