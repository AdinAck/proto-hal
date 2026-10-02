//! Recovery from failures to parse a node within a sequence.

use chumsky::prelude::*;

use super::{Error, Input};
use crate::{
    token::token,
    util::{Parsed, Spanned},
};

/// A [`Parsed`] node within a sequence that `end`s. On failure, the tokens that `skip` parses up to where the
/// sequence's next node would parse, or where it ends, become an [`Error`](Parsed::Error). Failing to reach either,
/// the failure stands.
pub(super) fn parsed<'tokens, 'src: 'tokens, T: 'tokens>(
    node: impl Parser<'tokens, Input<'tokens, 'src>, T, Error<'src>> + Clone + 'tokens,
    skip: impl Parser<'tokens, Input<'tokens, 'src>, (), Error<'src>> + Clone + 'tokens,
    end: impl Parser<'tokens, Input<'tokens, 'src>, (), Error<'src>> + Clone + 'tokens,
) -> impl Parser<'tokens, Input<'tokens, 'src>, Spanned<Parsed<T>>, Error<'src>> + Clone {
    // note: boxed, as both are used twice, which would otherwise compound in the types of nested sequences
    let node = node.boxed();
    // the next node, or the end of the sequence
    let resume = choice((node.clone().ignored(), end)).boxed();

    node.map(Parsed::Node)
        .recover_with(via_parser(
            skip.and_is(resume.clone().not())
                .repeated()
                .at_least(1)
                .then(resume.rewind())
                .map(|_| Parsed::Error),
        ))
        .spanned()
}

/// A token skipped in recovery, or a group of them balanced in their delimiters, so that skipping never ends within
/// a group.
pub(super) fn skipped<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, (), Error<'src>> + Clone {
    recursive(|skipped| {
        let group = |open, close| {
            just(open)
                .then(skipped.clone().and_is(just(close).not()).repeated())
                .then(just(close))
                .ignored()
        };

        choice((
            group(token![LBrace], token![RBrace]),
            group(token![LBracket], token![RBracket]),
            any().ignored(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use crate::parse;

    #[test]
    fn recovery() {
        for (s, expected, failures) in [
            (
                "register foo @ 0x0\n@\nregister bar @ 0x4\n!\nregister baz",
                "register foo @ 0x0\n    <error>\n\nregister bar @ 0x4\n    <error>\n\nregister baz",
                2,
            ),
            (
                "register foo { reed write field bar @ 0 field baz @ 1 }",
                "register foo {\n    <error> write field bar @ 0\n    field baz @ 1\n}",
                1,
            ),
            (
                "peripheral foo { register bar { field baz @ , } register qux @ 0x4 }",
                "peripheral foo {\n    register bar {\n        field baz\n            <error>\n    }\n    register qux @ 0x4\n}",
                1,
            ),
            (
                "register foo { reed { field bar @ 0 } field baz @ 1 }",
                "register foo {\n    <error>\n    field baz @ 1\n}",
                1,
            ),
            (
                "device foo { interrupts { bar @ 0 ! baz @ 1 } }",
                "device foo {\n    interrupts {\n        bar @ 0\n            <error>\n        baz @ 1\n    }\n}",
                1,
            ),
            (
                "register foo @ [0, @, 2]",
                "register foo @ [0, <error>, 2]",
                1,
            ),
            ("register foo @ [0 1]", "register foo @ [<error>]", 1),
            (
                "schema foo { leaky inrt variant Bar ~ 0 }",
                "schema foo {\n    leaky <error> variant Bar ~ 0\n}",
                1,
            ),
            (
                "field foo @ , reset 0 { variant Bar ~ 0 }",
                "field foo\n    <error>\n    reset 0\n{\n    variant Bar ~ 0\n}",
                1,
            ),
            (
                "schema foo { varient Bar ~ 0 variant Baz ~ 1 }",
                "schema foo {\n    <error>\n    variant Baz ~ 1\n}",
                1,
            ),
        ] {
            let (file, diagnostics) = parse(s, 0);

            assert_eq!(
                file.to_string(),
                expected,
                "'{s}' should recover to what parses around its failures",
            );
            assert_eq!(
                diagnostics.len(),
                failures,
                "'{s}' should fail exactly where it is erroneous",
            );
        }
    }

    #[test]
    fn near_misses() {
        for (s, note) in [
            (
                "register foo { reed write field bar @ 0 }",
                "a similarly named keyword `read` exists",
            ),
            (
                "schema foo { leaky inrt variant Bar ~ 0 }",
                "a similarly named keyword `inert` exists",
            ),
            (
                "schema foo { varient Bar ~ 0 }",
                "a similarly named keyword `variant` exists",
            ),
            (
                "field foo @ 0 resett 0",
                "a similarly named keyword `reset` exists",
            ),
        ] {
            let (.., diagnostics) = parse(s, 0);

            let notes: Vec<_> = diagnostics
                .into_iter()
                .flat_map(|diagnostic| diagnostic.notes)
                .collect();

            assert_eq!(
                notes,
                [note],
                "the misspelled keyword in '{s}' should be noted as a near miss",
            );
        }
    }
}
