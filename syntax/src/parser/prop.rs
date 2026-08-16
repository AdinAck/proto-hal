//! Head property parsers: everything that may appear between a definition's
//! name and its body.

use chumsky::prelude::*;

use super::{
    Extra, TokenInput,
    atom::{braced_set, bracketed_list, ident, number, path, range},
};
use crate::{
    ast::{
        Domain, Entitled, ListEntry, Pattern, ResetValue, Rest, Segment, Space, Spanned, Stride,
        ValueEntry, VariantValue,
    },
    token::token,
};

/// `...`, `...+0x4`, or `...-0x4` — continue the pattern, optionally with an
/// explicit stride.
pub(crate) fn rest<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Rest, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![...])
        .ignore_then(
            choice((just(token![+]).to(false), just(token![-]).to(true)))
                .then(number())
                .map(|(negative, magnitude)| Stride {
                    negative,
                    magnitude,
                })
                .spanned()
                .or_not(),
        )
        .map(|stride| Rest { stride })
}

/// `@ ...` — an address, offset, bit domain, or a list of them.
pub(crate) fn domain<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Domain>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let entry = choice((
        rest().map(ListEntry::Rest),
        range().map(ListEntry::Range),
        number().map(ListEntry::Value),
    ));

    just(token![@])
        .ignore_then(
            choice((
                bracketed_list(entry).map(Domain::List),
                range().map(Domain::Range),
                number().map(Domain::Value),
            ))
            .spanned()
            .labelled("domain"),
        )
        .or_not()
}

/// `assumes path` — the field exactly reflects the referenced schema.
pub(crate) fn assumes<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<crate::ast::Path<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Assumes]).ignore_then(path().spanned()).or_not()
}

/// `extends a, b` clauses — the schemas a field copies its variants from.
/// A field may extend many: comma lists and repeated clauses accumulate.
pub(crate) fn extends<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Vec<Spanned<crate::ast::Path<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Extends])
        .ignore_then(
            path()
                .spanned()
                .separated_by(just(token![,]))
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .repeated()
        .collect::<Vec<_>>()
        .map(|clauses: Vec<Vec<_>>| clauses.into_iter().flatten().collect())
}

/// `reset 0x7f` or `reset Disabled`.
pub(crate) fn reset<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<ResetValue<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Reset])
        .ignore_then(
            choice((
                number().map(ResetValue::Value),
                ident().map(ResetValue::Variant),
            ))
            .spanned()
            .labelled("reset value"),
        )
        .or_not()
}

/// `~ 0x3` or `~ [0x0, ...+0x4]` — the value(s) a variant occupies.
pub(crate) fn value<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<VariantValue>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let entry = choice((
        rest().map(ValueEntry::Rest),
        number().map(ValueEntry::Value),
    ));

    just(token![~])
        .ignore_then(
            choice((
                bracketed_list(entry).map(VariantValue::List),
                number().map(VariantValue::Value),
            ))
            .spanned()
            .labelled("value"),
        )
        .or_not()
}

/// A requirement space in disjunctive form: patterns of `&`-joined
/// entitlements, joined by `|`, with parentheses permitted (only) around each
/// pattern.
pub(crate) fn space<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Space<'src>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    let segment = ident()
        .then(
            range()
                .delimited_by(just(token![LBracket]), just(token![RBracket]))
                .or_not(),
        )
        .map(|(name, elements)| Segment { name, elements })
        .spanned();

    let entitled = segment
        .separated_by(just(token![.]))
        .at_least(1)
        .collect()
        .then(
            just(token![.])
                .ignore_then(braced_set(ident()).labelled("variant set"))
                .or_not(),
        )
        .map(|(segments, set)| Entitled { segments, set });

    let pattern = entitled
        .spanned()
        .separated_by(just(token![&]))
        .at_least(1)
        .collect()
        .map(|entitlements| Pattern { entitlements });

    choice((
        pattern
            .clone()
            .delimited_by(just(token![LParen]), just(token![RParen])),
        pattern,
    ))
    .spanned()
    .separated_by(just(token![|]))
    .at_least(1)
    .collect()
    .map(|patterns| Space { patterns })
    .labelled("requirement")
}

/// `requires ...` — statewise entitlements on a variant; ontological
/// entitlements elsewhere.
pub(crate) fn requires<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Space<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Requires])
        .ignore_then(space().spanned())
        .or_not()
}

/// `write requires ...` — write access entitlements.
pub(crate) fn write_requires<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Space<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Write])
        .ignore_then(just(token![Requires]))
        .ignore_then(space().spanned())
        .or_not()
}

/// `hardware write requires ...` — hardware write access entitlements.
pub(crate) fn hardware_write_requires<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, Option<Spanned<Space<'src>>>, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    just(token![Hardware])
        .ignore_then(just(token![Write]))
        .ignore_then(just(token![Requires]))
        .ignore_then(space().spanned())
        .or_not()
}
