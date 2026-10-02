//! Parsing of [declarations](Declaration).

use chumsky::prelude::*;

use super::{
    Error, Input, doc, ident,
    interrupt::interrupts,
    iteration::binding,
    name::name,
    property::property,
    recovery::{parsed, skipped},
};
use crate::{
    ast::{AccessModality, Child, Declaration, DeclarationKind, Qualifier},
    diagnostic::Expected,
    token::{keyword, token},
};

/// A [`Declaration`].
///
/// ```text
/// declaration ::= DOC* qualifier* kind name binding? property* body?
/// body        ::= "{" child* "}"
/// child       ::= declaration | interrupts
/// ```
pub(super) fn declaration<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Declaration<'src>, Error<'src>> + Clone {
    recursive(|declaration| {
        // declaration or interrupt table
        let child = choice((
            declaration.map(Child::Declaration),
            interrupts().map(Child::Interrupts),
        ))
        .labelled(Expected::Declaration);

        // { ... }
        let body = parsed(child, skipped(), just(token![RBrace]).ignored())
            .repeated()
            .collect()
            .delimited_by(just(token![LBrace]), just(token![RBrace]));

        // skipping words, as a misspelled qualifier is one, up to the kind
        let qualifiers = parsed(
            qualifier().labelled(Expected::Qualifier),
            ident().ignored(),
            kind().ignored(),
        )
        .repeated()
        .collect();

        // { or }, or what begins a sibling
        let properties_end = choice((
            one_of([token![LBrace], token![RBrace]]).ignored(),
            doc().ignored(),
            qualifier().ignored(),
            kind().ignored(),
            just(keyword![Interrupts]).ignored(),
            just(keyword![Import]).ignored(),
            end(),
        ));

        doc()
            .spanned()
            .repeated()
            .collect()
            .then(qualifiers)
            .then(kind().spanned())
            .then(name().spanned().labelled(Expected::Name))
            .then(binding().or_not())
            .then(
                parsed(
                    property().labelled(Expected::Property),
                    skipped(),
                    properties_end,
                )
                .repeated()
                .collect(),
            )
            .then(body.or_not())
            .map(
                |((((((docs, qualifiers), kind), name), variable), properties), children)| {
                    Declaration {
                        docs,
                        qualifiers,
                        kind,
                        name,
                        variable,
                        properties,
                        children: children.unwrap_or_default(),
                    }
                },
            )
    })
}

/// A [`Qualifier`].
///
/// ```text
/// qualifier ::= "leaky" | "inert" | access_modality
/// ```
fn qualifier<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Qualifier, Error<'src>> + Clone {
    choice((
        just(keyword![Leaky]).to(Qualifier::Leaky),
        just(keyword![Inert]).to(Qualifier::Inert),
        access_modality().map(Qualifier::AccessModality),
    ))
}

/// An [`AccessModality`].
///
/// ```text
/// access_modality ::= "read" "write"? | "write" | "volatile"? "store"
/// ```
fn access_modality<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, AccessModality, Error<'src>> + Clone {
    choice((
        just(keyword![Read])
            .ignore_then(just(keyword![Write]).or_not())
            .map(|write| match write {
                Some(..) => AccessModality::ReadWrite,
                None => AccessModality::Read,
            }),
        just(keyword![Write]).to(AccessModality::Write),
        just(keyword![Volatile])
            .ignore_then(just(keyword![Store]))
            .to(AccessModality::VolatileStore),
        just(keyword![Store]).to(AccessModality::Store),
    ))
}

/// A [`DeclarationKind`].
///
/// ```text
/// kind ::= "device" | "peripheral" | "register" | "field" | "schema" | "variant" | "group"
/// ```
fn kind<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, DeclarationKind, Error<'src>> + Clone {
    choice((
        just(keyword![Device]).to(DeclarationKind::Device),
        just(keyword![Peripheral]).to(DeclarationKind::Peripheral),
        just(keyword![Register]).to(DeclarationKind::Register),
        just(keyword![Field]).to(DeclarationKind::Field),
        just(keyword![Schema]).to(DeclarationKind::Schema),
        just(keyword![Variant]).to(DeclarationKind::Variant),
        just(keyword![Group]).to(DeclarationKind::Group),
    ))
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{AccessModality, Qualifier},
        diagnostic::{Expected, Found},
        parser::{
            declaration::declaration,
            tests::{expectations, parse},
        },
        token::token,
        util::Parsed,
    };

    #[test]
    fn declarations() {
        for s in [
            "register foo @ 0x0",
            "leaky store field foo[0..16] as bar @ [1:0, ...]\n    assumes baz",
            "read write field foo @ 0\n    write requires bar.Baz",
            "inert write variant Foo ~ 0",
            "variant [Foo, Bar] ~ [0, 1]\n    requires baz.Qux",
            "peripheral foo[a, b] as bar @ [0x4800_0000, ... by 0x400]\n    refines baz",
            "register foo @ 0x4\n    reset 0\n{\n    store field bar @ 0\n}",
            "/// Foo.\n/// Bar.\nschema foo {\n    variant Bar ~ 0\n    variant Baz ~ 1\n}",
            "group foo[1, ...] @ [0x8, ... by 0x14] {\n    register bar @ 0x0\n    register baz @ 0x4\n}",
            "register foo {\n    group bar[1..=6]\n}",
            "device foo {\n    peripheral bar @ 0x4000_0000 {\n        register baz @ 0x0 {\n            store field \
             qux @ 0\n        }\n    }\n}",
            "device foo {\n    interrupts {\n        /// Foo.\n        bar @ 0\n    }\n    peripheral baz @ 0x0\n}",
        ] {
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(declaration(), s).unwrap().to_string(),
                "expected display impl for {s:?} to reproduce the source text",
            );
        }
    }

    #[test]
    fn lines() {
        let s = "register foo @ 0x0 reset 0 { field bar @ 0 }";

        similar_asserts::assert_eq!(
            expected: "register foo @ 0x0\n    reset 0\n{\n    field bar @ 0\n}",
            found: parse!(declaration(), s).unwrap().to_string(),
            "expected display impl for the declaration parsed from `{s}` to break lines and indent canonically",
        );
    }

    #[test]
    fn intrinsic() {
        let s = "peripheral foo refines bar @ 0x0";

        similar_asserts::assert_eq!(
            expected: "peripheral foo @ 0x0\n    refines bar",
            found: parse!(declaration(), s).unwrap().to_string(),
            "expected display impl for the declaration parsed from `{s}` to display intrinsic properties on the head \
             line",
        );
    }

    #[test]
    fn empty_body() {
        let s = "register foo {}";

        similar_asserts::assert_eq!(
            expected: "register foo",
            found: parse!(declaration(), s).unwrap().to_string(),
            "expected display impl for the declaration parsed from `{s}` to omit the empty body",
        );
    }

    #[test]
    fn write() {
        let s = "schema foo { variant Bar ~ 0 write variant Baz ~ 1 }";

        similar_asserts::assert_eq!(
            expected: vec!["variant Bar ~ 0", "write variant Baz ~ 1"],
            found: parse!(declaration(), s)
                .unwrap()
                .children
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            "expected `write` to begin the next declaration in `{s}`, as `requires` doesn't follow",
        );
    }

    #[test]
    fn qualifiers() {
        for (s, expected) in [
            ("field foo", vec![]),
            (
                "leaky inert volatile store field foo",
                vec![
                    Qualifier::Leaky,
                    Qualifier::Inert,
                    AccessModality::VolatileStore.into(),
                ],
            ),
            (
                "read write field foo",
                vec![AccessModality::ReadWrite.into()],
            ),
            (
                "inert leaky field foo",
                vec![Qualifier::Inert, Qualifier::Leaky],
            ),
        ] {
            let qualifiers = parse!(declaration(), s)
                .unwrap()
                .qualifiers
                .into_iter()
                .map(|qualifier| *qualifier)
                .collect::<Vec<_>>();

            similar_asserts::assert_eq!(
                expected: Vec::from_iter(expected.into_iter().map(Parsed::Node)),
                found: qualifiers,
                "expected the qualifiers parsed from `{s}` to be listed in the order written",
            );
        }
    }

    #[test]
    fn expected() {
        for (s, found, expected) in [
            ("register @", Found::Token(token![@]), vec![Expected::Name]),
            (
                "register foo { @ }",
                Found::Token(token![@]),
                vec![Expected::Declaration],
            ),
        ] {
            similar_asserts::assert_eq!(
                expected: vec![(found, expected.clone())],
                found: expectations(parse!(declaration(), s).unwrap_err()),
                "expected parsing `{s}` as a declaration to fail, expecting {expected:?}",
            );
        }
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "foo",
            "field",
            "store volatile field foo",
            "register foo {",
            "register foo { bar @ 0 }",
            "interrupts { foo @ 0 }",
        ] {
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(declaration(), s).ok().map(|declaration| declaration.to_string()),
                "expected parsing `{s}` as a declaration to fail",
            );
        }
    }
}
