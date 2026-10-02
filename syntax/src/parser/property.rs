//! Parsing of [properties](Property).

use chumsky::prelude::*;

use super::{
    Error, Input, entitlement::entitlement_space, list::list, path::module_path, value::value,
};
use crate::{
    ast::{Argument, Property, RequirementQualifiers},
    diagnostic::Expected,
    token::{keyword, token},
};

/// A [`Property`].
///
/// ```text
/// property ::= "@" argument | "~" argument | "reset" argument
///            | "assumes" module_path
///            | "refines" module_path ("," module_path)*
///            | ("hardware"? "write")? "requires" entitlement_space
/// ```
pub(super) fn property<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Property<'src>, Error<'src>> + Clone {
    // after @, ~, or reset
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
            .ignore_then(argument)
            .map(Property::Reset),
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
    use std::assert_matches;

    use crate::{
        ast::{Property, RequirementQualifiers},
        diagnostic::{Expected, Found},
        parser::{Failure, property::property, tests::parse},
        token::{keyword, token},
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
            "assumes foo",
            "assumes foo::bar",
            "refines foo",
            "refines foo, bar::baz",
            "requires foo.Bar",
            "write requires foo.Bar & baz.Qux",
            "hardware write requires foo.{Bar, Baz} | qux.Foo",
        ] {
            assert_eq!(
                parse!(property(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
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
            assert_matches!(
                parse!(property(), s).unwrap(),
                Property::Requires { qualifiers, .. } if qualifiers == expected,
                "'{s}' should have the qualifiers {expected:?}",
            );
        }
    }

    #[test]
    fn expected() {
        for (s, expected_found, expected_symbol) in [
            ("@", Found::End, Expected::Value),
            ("~ ,", Found::Token(token![,]), Expected::Value),
            ("reset @", Found::Token(token![@]), Expected::Value),
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
            assert_matches!(
                parse!(property(), s).unwrap_err().as_slice(),
                [Failure { found, expected, .. }]
                    if *found == expected_found && expected.iter().eq(&[expected_symbol]),
                "'{s}' should expect {expected_symbol}",
            );
        }
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "@ foo.bar",
            "reset",
            "assumes foo, bar",
            "assumes foo.bar",
            "refines",
            "refines foo,, bar",
            "write",
            "write hardware requires foo.Bar",
            "requires",
        ] {
            assert!(
                parse!(property(), s).is_err(),
                "'{s}' was parsed as a property when it shouldn't be",
            );
        }
    }
}
