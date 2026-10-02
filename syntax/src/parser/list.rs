//! Parsing of [lists](List) and their [entries](Entry).

use chumsky::prelude::*;

use super::{
    Error, Input,
    iteration::{range, step, variable},
    recovery::{parsed, skipped},
    set::set,
    value::value,
};
use crate::{
    ast::{
        List,
        list::{Entry, Progression},
    },
    token::{keyword, token},
    util::{Parsed, Spanned},
};

/// A [`List`].
///
/// ```text
/// list       ::= "[" list_entry ("," list_entry)* ("," progression)? ","? "]"
/// list_entry ::= range | value | list | set
/// ```
pub(super) fn list<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, List<'src>, Error<'src>> + Clone {
    recursive(|list| {
        let comma = just(token![,]);

        // range, value, nested list, or set
        let entry = choice((
            range().map(Entry::Range),
            value().map(Entry::Value),
            list.map(Entry::List),
            set().map(Entry::Set),
        ));

        // , ] or ...
        let end = one_of([token![,], token![RBracket], token![...]]).ignored();

        parsed(entry, skipped(), end)
            .separated_by(comma)
            .at_least(1)
            .collect()
            .then(comma.ignore_then(progression().spanned()).or_not())
            .then_ignore(comma.or_not())
            .delimited_by(just(token![LBracket]), just(token![RBracket]))
            // note: a list that fails to parse entry by entry, i.e. for a missing comma, is a list of one error
            .recover_with(via_parser(nested_delimiters(
                token![LBracket],
                token![RBracket],
                [(token![LBrace], token![RBrace])],
                |span| {
                    (
                        vec![Spanned {
                            inner: Parsed::Error,
                            span,
                        }],
                        None,
                    )
                },
            )))
            .map(|(entries, progression)| List {
                entries,
                progression,
            })
    })
}

/// A [`Progression`].
///
/// ```text
/// progression ::= "..." step? ("in" variable)?
/// ```
fn progression<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Progression<'src>, Error<'src>> + Clone {
    just(token![...])
        .ignore_then(step().or_not())
        .then(
            just(keyword![In])
                .ignore_then(variable().spanned())
                .or_not(),
        )
        .map(|(step, scope)| Progression { step, scope })
}

#[cfg(test)]
mod tests {
    use crate::parser::{list::list, tests::parse};

    #[test]
    fn lists() {
        for s in [
            "[0]",
            "[a, b, c]",
            "[3:0, 7:4]",
            "[0..16, 0..=15 by 4]",
            "[[0, 2..=7], ...]",
            "[{1, 2}, {3..=5}]",
            "[8, ...]",
            "[1:0, ... by -4]",
            "[0..=7, ... in #foo]",
            "[0..=5, ... by 16 in #bar]",
            "[0x8, ... by 0x14]",
            "[0b1..=0b11 by -0o4, 0x0400_0000]",
        ] {
            assert_eq!(
                parse!(list(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn trailing_comma() {
        for (parsed, expected) in [
            (parse!(list(), "[a, b,]").unwrap().to_string(), "[a, b]"),
            (parse!(list(), "[a, ...,]").unwrap().to_string(), "[a, ...]"),
        ] {
            assert_eq!(parsed, expected, "trailing comma should not be retained");
        }
    }

    #[test]
    fn spans() {
        let list = parse!(list(), "[foo, 0..=7]").unwrap();

        let spans: Vec<_> = list
            .entries
            .iter()
            .map(|entry| entry.span.range())
            .collect();

        assert_eq!(spans, [1..4, 6..11], "produced spans were not as expected");
    }

    #[test]
    fn reject_lists() {
        for s in [
            "[]",
            "[...]",
            "[0, ..., 1]",
            "[0, ..., ...]",
            "[0,, 1]",
            "[, 0]",
            "[0 1]",
            "[0..]",
            "[..=7]",
            "[0 by 4]",
            "[0, ... in foo]",
            "[0, ... in #foo by 4]",
            "[0, by -4]",
            "[#]",
            "[#foo]",
            "[0",
        ] {
            assert!(
                parse!(list(), s).is_err(),
                "'{s}' was parsed as a list when it shouldn't be"
            );
        }
    }
}
