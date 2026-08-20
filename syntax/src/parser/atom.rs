//! The small parsers most rules are composed from.

use chumsky::prelude::*;

use super::{Extra, TokenInput};
use crate::{
    ast::{Access, Head, Indices, NumRange, Path, Side, Span, Spanned},
    token::{Token, token},
};

/// A name.
pub fn ident<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, &'src str, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Ident(s) => s }.labelled("identifier")
}

/// An integer literal.
pub fn literal<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, u32, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Literal(n) => n }.labelled("integer literal")
}

/// The doc comments preceding an item.
pub fn docs<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Spanned<&'src str>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Doc(s) => s }
        .labelled("doc comment")
        .spanned()
        .repeated()
        .collect()
}

/// A `.`-separated reference.
pub fn path<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Path<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    ident()
        .spanned()
        .separated_by(just(token![.]))
        .at_least(1)
        .collect()
        .map(|segments| Path { segments })
        .labelled("path")
}

/// `0..2`, `0..=1`, `0..=4 by 2`.
pub fn range<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, NumRange, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    literal()
        .spanned()
        .then(select! { token![..] => false, token![..=] => true }.labelled("range operator"))
        .then(literal().spanned())
        .then(just(token![By]).ignore_then(literal().spanned()).or_not())
        .map(|(((start, inclusive), end), step)| NumRange {
            start,
            end,
            inclusive,
            step,
        })
        .labelled("range")
}

/// An optional marker keyword (`leaky`, `inert`, `array`), captured as the
/// span it occupies.
pub fn marker<'tokens, 'src: 'tokens, I>(
    token: Token<'src>,
) -> impl Parser<'tokens, I, Option<Span>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token)
        .to(())
        .spanned()
        .map(|s: Spanned<()>| s.span)
        .or_not()
}

/// The access modality set.
///
/// Modalities compose as a set, so `read write` and `write read` are the same
/// thing and each word may appear at most once — `read read` is a syntax
/// error by construction.
pub fn access<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Spanned<Access>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let read = just(token![Read]).to(Access::Read).spanned();
    let write = just(token![Write]).to(Access::Write).spanned();

    choice((
        just(token![Volatile])
            .then(just(token![Store]))
            .to(Access::VolatileStore)
            .spanned()
            .map(|a| vec![a]),
        just(token![Store])
            .to(Access::Store)
            .spanned()
            .map(|a| vec![a]),
        read.then(write.or_not())
            .map(|(r, w)| std::iter::once(r).chain(w).collect()),
        write
            .then(read.or_not())
            .map(|(w, r)| std::iter::once(w).chain(r).collect()),
    ))
    .labelled("access modality")
    .or_not()
    .map(Option::unwrap_or_default)
}

/// `read` or `write` — the numericity a variant occupies within a
/// `read write` container. Only a single side may be named: a variant either
/// belongs to one numericity or (unmarked) to all of them.
pub fn side<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Side>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        just(token![Read]).to(Side::Read),
        just(token![Write]).to(Side::Write),
    ))
    .spanned()
    .labelled("variant side")
    .or_not()
}

/// A bracketed, comma-separated list, with each entry spanned.
pub fn bracketed_list<'tokens, 'src: 'tokens, I, T>(
    entry: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Vec<Spanned<T>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    entry
        .spanned()
        .separated_by(just(token![,]))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(token![LBracket]), just(token![RBracket]))
}

/// A braced, comma-separated set, with each entry spanned.
///
/// Curly braces mark *membership*: unlike a bracketed list, the entries are
/// unordered and never zip against a parallel list.
pub fn braced_set<'tokens, 'src: 'tokens, I, T>(
    entry: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Vec<Spanned<T>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    entry
        .spanned()
        .separated_by(just(token![,]))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(token![LBrace]), just(token![RBrace]))
}

/// Array element designators: `[a, b, c]` or `[0..=15]`.
pub fn indices<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<Indices<'src>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        range()
            .then(just(token![,]).ignore_then(just(token![...])).or_not())
            .map(|(range, series)| match series {
                Some(..) => Indices::Series(range),
                None => Indices::Range(range),
            })
            .delimited_by(just(token![LBracket]), just(token![RBracket])),
        bracketed_list(ident()).map(Indices::Names),
    ))
    .spanned()
    .labelled("array indices")
}

/// `#template`, `#template as name`, or `name`.
pub fn plain_head<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Head<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        just(token![#])
            .ignore_then(path().spanned())
            .then(just(token![As]).ignore_then(ident().spanned()).or_not())
            .map(|(template, name)| Head {
                template: Some(template),
                name,
                indices: None,
            }),
        ident().spanned().map(|name| Head {
            template: None,
            name: Some(name),
            indices: None,
        }),
    ))
    .or_not()
    .map(Option::unwrap_or_default)
}

/// As [`plain_head`], where names may carry array [`indices`].
pub fn indexed_head<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Head<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let name_part = ident().spanned().then(indices().or_not());

    choice((
        just(token![#])
            .ignore_then(path().spanned())
            .then(just(token![As]).ignore_then(name_part.clone()).or_not())
            .map(|(template, name_part)| match name_part {
                Some((name, indices)) => Head {
                    template: Some(template),
                    name: Some(name),
                    indices,
                },
                None => Head {
                    template: Some(template),
                    name: None,
                    indices: None,
                },
            }),
        name_part.map(|(name, indices)| Head {
            template: None,
            name: Some(name),
            indices,
        }),
    ))
    .or_not()
    .map(Option::unwrap_or_default)
}

/// A braced list of items, recovering from an unrecoverable inner error by
/// discarding the whole (brace-balanced) body.
pub fn body<'tokens, 'src: 'tokens, I, T>(
    item: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Spanned<Vec<Spanned<T>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    item.spanned()
        .repeated()
        .collect()
        .delimited_by(just(token![LBrace]), just(token![RBrace]))
        .spanned()
        .recover_with(via_parser(nested_delimiters(
            token![LBrace],
            token![RBrace],
            [
                (token![LParen], token![RParen]),
                (token![LBracket], token![RBracket]),
            ],
            |span| Spanned {
                inner: Vec::new(),
                span,
            },
        )))
}
