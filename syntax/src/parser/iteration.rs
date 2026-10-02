//! Parsing of [ranges](Range), [steps](Step), and iteration variables.

use chumsky::prelude::*;

use super::{Error, Input, ident, literal};
use crate::{
    ast::{Range, Step},
    diagnostic::Expected,
    token::{Ident, keyword, token},
    util::Spanned,
};

/// A [`Range`].
///
/// ```text
/// range ::= LITERAL (".." | "..=") LITERAL step?
/// ```
pub(super) fn range<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Range<'src>, Error<'src>> + Clone {
    literal()
        .then(choice((
            just(token![..]).to(false),
            just(token![..=]).to(true),
        )))
        .then(literal())
        .then(step().or_not())
        .map(|(((start, inclusive), end), step)| Range {
            start,
            end,
            inclusive,
            step,
        })
}

/// A [`Step`].
///
/// ```text
/// step ::= "by" "-"? LITERAL
/// ```
pub(super) fn step<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Step<'src>, Error<'src>> + Clone {
    just(keyword![By])
        .ignore_then(just(token![-]).or_not())
        .then(literal())
        .map(|(minus, distance)| Step {
            negative: minus.is_some(),
            distance,
        })
}

/// An iteration variable i.e. `#foo`.
///
/// ```text
/// variable ::= "#" IDENT
/// ```
pub(super) fn variable<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Ident<'src>, Error<'src>> + Clone {
    just(token![#]).ignore_then(ident().labelled(Expected::IterationVariable))
}

/// An iteration variable's binding, naming an iteration i.e. `as foo`.
///
/// ```text
/// binding ::= "as" IDENT
/// ```
pub(super) fn binding<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Spanned<Ident<'src>>, Error<'src>> + Clone {
    just(keyword![As]).ignore_then(ident().labelled(Expected::IterationVariable).spanned())
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        diagnostic::{Expected, Found},
        parser::{Failure, list::list, tests::parse},
        token::{literal, token},
    };

    #[test]
    fn variable_whitespace() {
        assert_eq!(
            parse!(list(), "[0, ... in # foo]").unwrap().to_string(),
            "[0, ... in #foo]",
            "whitespace after `#` should not be retained"
        );
    }

    #[test]
    fn variable_expected() {
        for (s, expected_found) in [
            ("[0, ... in #0]", Found::Token(literal![0])),
            ("[0, ... in #]", Found::Token(token![RBracket])),
        ] {
            assert_matches!(
                parse!(list(), s).unwrap_err().as_slice(),
                [Failure { found, expected, .. }]
                    if *found == expected_found && expected.iter().eq(&[Expected::IterationVariable]),
                "'{s}' should expect an iteration variable",
            );
        }
    }
}
