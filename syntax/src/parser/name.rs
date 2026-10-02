//! Parsing of [names](crate::ast::Name).

use chumsky::{input::InputRef, prelude::*};

use super::{Error, Input, ident, iteration::variable, list::list, set::set};
use crate::{
    ast::{Fragment, Name},
    token::{Hash, LBrace, LBracket, Token, token},
};

/// A [`Name`]. Adjacent [fragments](Fragment) are of different kinds, so a fragment of the same kind as the one
/// before it ends the name, as does a `{` that does not begin a set, i.e. a body.
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
                // such as a body's `{`, which does not begin a set
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
    /// Determine the kind of the provided fragment.
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
    use crate::{
        ast::Fragment,
        diagnostic::Found,
        parser::{name::name, tests::parse},
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
            similar_asserts::assert_eq!(
                expected: s,
                found: parse!(name(), s).unwrap().to_string(),
                "expected display impl for `{s}` to reproduce the source text",
            );
        }
    }

    #[test]
    fn spans() {
        let s = "foo[1..=3]bar";

        let name = parse!(name(), s).unwrap();

        similar_asserts::assert_eq!(
            expected: vec![0..3, 3..10, 10..13],
            found: name
                .fragments
                .iter()
                .map(|fragment| fragment.span.range())
                .collect::<Vec<_>>(),
            "expected the spans produced by parsing `{s}` to correspond to the name fragments",
        );
    }

    #[test]
    fn insertion() {
        let s = "foo[#bar]baz";

        let name = parse!(name(), s).unwrap();

        similar_asserts::assert_eq!(
            expected: &Fragment::Insertion(Ident("bar")),
            found: &*name.fragments[1],
            "expected `[#bar]` in `{s}` to be parsed as an insertion",
        );
    }

    #[test]
    fn whitespace() {
        for (s, expected) in [
            ("foo [a]", "foo[a]"),
            ("foo[a]  bar", "foo[a]bar"),
            ("foo [1..=3] bar", "foo[1..=3]bar"),
        ] {
            similar_asserts::assert_eq!(
                expected: expected,
                found: parse!(name(), s).unwrap().to_string(),
                "expected display impl for the name parsed from `{s}` to omit the whitespace",
            );
        }
    }

    #[test]
    fn adjacent_fragments() {
        for (s, found) in [
            ("foo bar", ident!["bar"]),
            ("foo[a][b]", token![LBracket]),
            ("{a}{b}", token![LBrace]),
        ] {
            similar_asserts::assert_eq!(
                expected: vec![Found::Token(found)],
                found: parse!(name(), s)
                    .unwrap_err()
                    .into_iter()
                    .map(|failure| failure.found)
                    .collect::<Vec<_>>(),
                "expected a fragment of the same kind as the fragment before it to end the name in `{s}`",
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
            similar_asserts::assert_eq!(
                expected: None,
                found: parse!(name(), s).ok().map(|name| name.to_string()),
                "expected parsing `{s}` as a name to fail",
            );
        }
    }
}
