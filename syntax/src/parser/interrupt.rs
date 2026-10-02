//! Parsing of [interrupt tables](InterruptTable).

use chumsky::prelude::*;

use super::{
    Error, Input, doc, ident,
    iteration::binding,
    name::name,
    property::property,
    recovery::{parsed, skipped},
};
use crate::{
    ast::{Interrupt, InterruptTable},
    diagnostic::Expected,
    token::{keyword, token},
};

/// An [`InterruptTable`].
///
/// ```text
/// interrupts ::= "interrupts" "{" interrupt* "}"
/// ```
pub(super) fn interrupts<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, InterruptTable<'src>, Error<'src>> + Clone {
    let interrupt = interrupt().labelled(Expected::Interrupt);

    just(keyword![Interrupts])
        .ignore_then(
            parsed(interrupt, skipped(), just(token![RBrace]).ignored())
                .repeated()
                .collect()
                .delimited_by(just(token![LBrace]), just(token![RBrace])),
        )
        .map(|interrupts| InterruptTable { interrupts })
}

/// An [`Interrupt`].
///
/// ```text
/// interrupt ::= DOC* name binding? property*
/// ```
fn interrupt<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Interrupt<'src>, Error<'src>> + Clone {
    // } or what begins a sibling
    let properties_end = choice((
        just(token![RBrace]).ignored(),
        doc().ignored(),
        ident().ignored(),
        end(),
    ));

    doc()
        .spanned()
        .repeated()
        .collect()
        .then(name().spanned())
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
        .map(|(((docs, name), variable), properties)| Interrupt {
            docs,
            name,
            variable,
            properties,
        })
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{Failure, interrupt::interrupts, tests::parse},
        token::token,
    };

    #[test]
    fn tables() {
        for s in [
            "interrupts {}",
            "interrupts {\n    foo @ 0\n}",
            "interrupts {\n    foo @ 0\n    bar[0..=4] @ [6, ...]\n}",
            "interrupts {\n    /// Foo.\n    foo @ 0\n}",
            "interrupts {\n    foo[a, b] as bar @ [1, 2]\n}",
        ] {
            assert_eq!(
                parse!(interrupts(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn lines() {
        assert_eq!(
            parse!(interrupts(), "interrupts { foo @ 0 bar @ 1 }")
                .unwrap()
                .to_string(),
            "interrupts {\n    foo @ 0\n    bar @ 1\n}",
            "interrupts should display one per line",
        );
    }

    #[test]
    fn expected_interrupt() {
        assert_matches!(
            parse!(interrupts(), "interrupts { @ }").unwrap_err().as_slice(),
            [Failure { found: Found::Token(token![@]), expected, .. }]
                if expected.iter().eq(&[Expected::Interrupt]),
            "a table should expect an interrupt",
        );
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "interrupts",
            "interrupts {",
            "interrupts { register foo }",
            "interrupts { foo @ 0, bar @ 1 }",
        ] {
            assert!(
                parse!(interrupts(), s).is_err(),
                "'{s}' was parsed as an interrupt table when it shouldn't be",
            );
        }
    }
}
