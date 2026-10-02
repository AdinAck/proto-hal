//! Parsing of [names](crate::ast::Name).

use chumsky::{input::InputRef, prelude::*};

use super::{Error, Input, ident, iteration::variable, list::list, set::set};
use crate::{
    ast::{Fragment, Name},
    token::{Hash, LBrace, LBracket, Token, token},
};

/// A [`Name`]. Adjacent [fragments](Fragment) are of different kinds, so a fragment of the same kind as the one
/// before it ends the name, as does a `{` that does not begin a set i.e. a body.
///
/// ```text
/// name ::= fragment+
/// ```
pub(super) fn name<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Name<'src>, Error<'src>> + Clone {
    // text, list, set, or insertion
    let fragment = fragment().spanned();

    custom(move |input| {
        let first = input.parse(&fragment)?;
        let mut last = FragmentKind::of(&first);
        let mut fragments = vec![first];

        while let Some(next) = FragmentKind::upcoming(input)
            && next != last
        {
            let before = input.save();

            match input.parse(&fragment) {
                Ok(fragment) => {
                    last = FragmentKind::of(&fragment);
                    fragments.push(fragment);
                }
                // i.e. a body's `{`, which is not a set
                Err(..) => {
                    input.rewind(before);
                    break;
                }
            }
        }

        Ok(Name { fragments })
    })
}

/// A fragment kind identifies the variant of a [`Fragment`], without its contents.
#[derive(PartialEq, Eq)]
enum FragmentKind {
    Text,
    List,
    Set,
    Insertion,
}

impl FragmentKind {
    /// Determine the kind of `fragment`.
    fn of(fragment: &Fragment) -> Self {
        match fragment {
            Fragment::Text(..) => Self::Text,
            Fragment::List(..) => Self::List,
            Fragment::Set(..) => Self::Set,
            Fragment::Insertion(..) => Self::Insertion,
        }
    }

    /// Determine the kind of fragment the input starts with from its first tokens, without parsing it.
    fn upcoming<'tokens, 'src: 'tokens>(
        input: &mut InputRef<'tokens, '_, Input<'tokens, 'src>, Error<'src>>,
    ) -> Option<Self> {
        let before = input.save();

        let kind = match input.next_maybe().as_deref() {
            Some(Token::Ident(..)) => Some(Self::Text),
            Some(Token::Punctuation(LBrace)) => Some(Self::Set),
            Some(Token::Punctuation(LBracket)) => match input.next_maybe().as_deref() {
                Some(Token::Operator(Hash)) => Some(Self::Insertion),
                _ => Some(Self::List),
            },
            _ => None,
        };

        input.rewind(before);

        kind
    }
}

/// A [`Fragment`].
///
/// ```text
/// fragment ::= IDENT | list | set | "[" variable "]"
/// ```
fn fragment<'tokens, 'src: 'tokens>()
-> impl Parser<'tokens, Input<'tokens, 'src>, Fragment<'src>, Error<'src>> + Clone {
    // note: an insertion is tried before a list, which would otherwise recover from it
    choice((
        ident().map(Fragment::Text),
        variable()
            .delimited_by(just(token![LBracket]), just(token![RBracket]))
            .map(Fragment::Insertion),
        list().map(Fragment::List),
        set().map(Fragment::Set),
    ))
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use crate::{
        ast::Fragment,
        diagnostic::Found,
        parser::{Failure, name::name, tests::parse},
        token::{Ident, ident, token},
    };

    #[test]
    fn names() {
        for s in [
            "foo",
            "foo[a, b, c]",
            "foo[0..16]",
            "foo[1..=3]bar",
            "[Foo, Bar]",
            "foo[12, 345]_bar",
            "foo[0..=5, ... in #bar]baz",
            "foo[#bar]baz",
            "foo{1, 2}",
            "{Foo, Bar}",
        ] {
            assert_eq!(
                parse!(name(), s).unwrap().to_string(),
                s,
                "'{s}' should display as written",
            );
        }
    }

    #[test]
    fn spans() {
        let name = parse!(name(), "foo[1..=3]bar").unwrap();

        let spans: Vec<_> = name
            .fragments
            .iter()
            .map(|fragment| fragment.span.range())
            .collect();

        assert_eq!(
            spans,
            [0..3, 3..10, 10..13],
            "produced spans were not as expected",
        );
    }

    #[test]
    fn insertion() {
        let name = parse!(name(), "foo[#bar]baz").unwrap();

        assert_matches!(
            *name.fragments[1],
            Fragment::Insertion(Ident("bar")),
            "`[#bar]` should be an insertion, not a list",
        );
    }

    #[test]
    fn whitespace() {
        for (s, expected) in [
            ("foo [a]", "foo[a]"),
            ("foo[a]  bar", "foo[a]bar"),
            ("foo [1..=3] bar", "foo[1..=3]bar"),
        ] {
            assert_eq!(
                parse!(name(), s).unwrap().to_string(),
                expected,
                "whitespace in '{s}' should not be retained"
            );
        }
    }

    #[test]
    fn adjacent_fragments() {
        for (s, expected) in [
            ("foo bar", ident!["bar"]),
            ("foo[a][b]", token![LBracket]),
            ("{a}{b}", token![LBrace]),
        ] {
            assert_matches!(
                parse!(name(), s).unwrap_err().as_slice(),
                [Failure { found: Found::Token(found), .. }] if *found == expected,
                "a fragment of the same kind as the one before it should end the name in '{s}'",
            );
        }
    }

    #[test]
    fn reject() {
        for s in [
            "",
            "0",
            "#foo",
            "foo.bar",
            "foo@",
            "foo[#bar, a]",
            "foo[0, #bar]",
            "foo{#bar}",
        ] {
            assert!(
                parse!(name(), s).is_err(),
                "'{s}' was parsed as a name when it shouldn't be",
            );
        }
    }
}
