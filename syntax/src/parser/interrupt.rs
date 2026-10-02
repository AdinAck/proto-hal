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
    use crate::{
        diagnostic::{Expected, Found},
        parser::{
            interrupt::interrupts,
            tests::{expectations, parse},
        },
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
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(interrupts(), s).unwrap().to_string(),
                "expected display impl for {s:?} to reproduce the source text",
            );
        }
    }

    #[test]
    fn lines() {
        let s = "interrupts { foo @ 0 bar @ 1 }";

        similar_asserts::assert_eq!(
            expected: "interrupts {\n    foo @ 0\n    bar @ 1\n}",
            found: parse!(interrupts(), s).unwrap().to_string(),
            "expected display impl for the interrupt table parsed from `{s}` to display one interrupt per line",
        );
    }

    #[test]
    fn expected_interrupt() {
        let s = "interrupts { @ }";

        similar_asserts::assert_eq!(
            expected: vec![(Found::Token(token![@]), vec![Expected::Interrupt])],
            found: expectations(parse!(interrupts(), s).unwrap_err()),
            "expected parsing `{s}` as an interrupt table to fail, expecting an interrupt",
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
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(interrupts(), s).ok().map(|table| table.to_string()),
                "expected parsing `{s}` as an interrupt table to fail",
            );
        }
    }
}
