//! The small parsers most rules are composed from.

use chumsky::prelude::*;

use super::{Extra, TokenInput};
use crate::{
    ast::{Access, Head, Indices, NumRange, Path, Side, Span, Spanned},
    token::Token,
};

/// A name.
pub(crate) fn ident<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, &'src str, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Ident(s) => s }.labelled("identifier")
}

/// A number literal of any radix.
pub(crate) fn number<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, u32, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    select! { Token::Num(n) => n }.labelled("number")
}

/// The doc comments preceding an item.
pub(crate) fn docs<'tokens, 'src: 'tokens, I>()
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
pub(crate) fn path<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Path<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    ident()
        .spanned()
        .separated_by(just(Token::Dot))
        .at_least(1)
        .collect()
        .map(|segments| Path { segments })
        .labelled("path")
}

/// `0..2` or `0..=1`.
pub(crate) fn range<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, NumRange, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    number()
        .spanned()
        .then(
            select! { Token::DotDot => false, Token::DotDotEq => true }.labelled("range operator"),
        )
        .then(number().spanned())
        .map(|((start, inclusive), end)| NumRange {
            start,
            end,
            inclusive,
        })
        .labelled("range")
}

/// An optional marker keyword (`leaky`, `inert`, `array`), captured as the
/// span it occupies.
pub(crate) fn marker<'tokens, 'src: 'tokens, I>(
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
pub(crate) fn access<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Spanned<Access>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let read = just(Token::Read).to(Access::Read).spanned();
    let write = just(Token::Write).to(Access::Write).spanned();

    choice((
        just(Token::Volatile)
            .then(just(Token::Store))
            .to(Access::VolatileStore)
            .spanned()
            .map(|a| vec![a]),
        just(Token::Store)
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
pub(crate) fn side<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Side>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        just(Token::Read).to(Side::Read),
        just(Token::Write).to(Side::Write),
    ))
    .spanned()
    .labelled("variant side")
    .or_not()
}

/// A bracketed, comma-separated list, with each entry spanned.
pub(crate) fn bracketed_list<'tokens, 'src: 'tokens, I, T>(
    entry: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Vec<Spanned<T>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    entry
        .spanned()
        .separated_by(just(Token::Comma))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
}

/// A braced, comma-separated set, with each entry spanned.
///
/// Curly braces mark *membership*: unlike a bracketed list, the entries are
/// unordered and never zip against a parallel list.
pub(crate) fn braced_set<'tokens, 'src: 'tokens, I, T>(
    entry: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Vec<Spanned<T>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    entry
        .spanned()
        .separated_by(just(Token::Comma))
        .at_least(1)
        .allow_trailing()
        .collect()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
}

/// Array element designators: `[a, b, c]` or `[0..=15]`.
pub(crate) fn indices<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Spanned<Indices<'src>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        range()
            .then(
                just(Token::Comma)
                    .ignore_then(just(Token::Ellipsis))
                    .or_not(),
            )
            .map(|(range, series)| match series {
                Some(..) => Indices::Series(range),
                None => Indices::Range(range),
            })
            .delimited_by(just(Token::LBracket), just(Token::RBracket)),
        bracketed_list(ident()).map(Indices::Names),
    ))
    .spanned()
    .labelled("array indices")
}

/// `#template`, `#template as name`, or `name`.
pub(crate) fn plain_head<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Head<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    choice((
        just(Token::Hash)
            .ignore_then(path().spanned())
            .then(just(Token::As).ignore_then(ident().spanned()).or_not())
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
pub(crate) fn indexed_head<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Head<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let name_part = ident().spanned().then(indices().or_not());

    choice((
        just(Token::Hash)
            .ignore_then(path().spanned())
            .then(just(Token::As).ignore_then(name_part.clone()).or_not())
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
pub(crate) fn body<'tokens, 'src: 'tokens, I, T>(
    item: impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone,
) -> impl Parser<'tokens, I, Spanned<Vec<Spanned<T>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    item.spanned()
        .repeated()
        .collect()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .spanned()
        .recover_with(via_parser(nested_delimiters(
            Token::LBrace,
            Token::RBrace,
            [
                (Token::LParen, Token::RParen),
                (Token::LBracket, Token::RBracket),
            ],
            |span| Spanned {
                inner: Vec::new(),
                span,
            },
        )))
}
