//! Error recovery: how the parser continues past a mistake to report the next
//! one.
//!
//! A word of caution from experience: chumsky's `skip_then_retry_until`
//! *rejects* any retried parse that itself contains a recovered inner error
//! (see its `secondary_errors_since` filter), which silently discards the
//! whole file when errors nest. The strategies here avoid retrying entirely.

use chumsky::prelude::*;

use super::{Extra, TokenInput};
use crate::token::{Token, keyword, token};

/// A brace-balanced block of arbitrary tokens.
pub(crate) fn block<'tokens, 'src: 'tokens, I>()
-> impl Parser<'tokens, I, (), Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
{
    recursive(|block| {
        let not_brace = any()
            .and_is(one_of([token![LBrace], token![RBrace]]).not())
            .ignored();

        just(token![LBrace])
            .ignore_then(choice((block, not_brace)).repeated())
            .then_ignore(just(token![RBrace]))
            .ignored()
    })
}

/// Consume a (nonempty) unparseable region up to the next plausible item or
/// enclosing closing brace — swallowing brace-balanced blocks whole — leaving
/// `fallback` in its place.
///
/// The original error (with its rich expectations) is preserved and reported;
/// errors *within* a swallowed block are suppressed, since a region already
/// known to be broken would only produce cascading noise.
pub(crate) fn garbage<'tokens, 'src: 'tokens, I, T>(
    fallback: T,
) -> impl Parser<'tokens, I, T, Extra<'tokens, 'src>> + Clone
where
    I: TokenInput<'tokens, 'src>,
    T: Clone,
{
    // tokens that plausibly begin an item
    let item_start = select! {
        Token::Doc(..) => (),
        keyword![Device] => (),
        keyword![Peripheral] => (),
        keyword![Register] => (),
        keyword![Field] => (),
        keyword![Schema] => (),
        keyword![Variant] => (),
        keyword![Import] => (),
        keyword![Interrupts] => (),
        keyword![Read] => (),
        keyword![Write] => (),
        keyword![Store] => (),
        keyword![Volatile] => (),
        keyword![Leaky] => (),
        keyword![Inert] => (),
    };

    let sync = item_start.or(one_of([token![LBrace], token![RBrace]]).ignored());

    // never consume a closing brace: it belongs to the enclosing body
    let first = choice((block(), any().and_is(just(token![RBrace]).not()).ignored()));

    let rest = choice((block(), any().and_is(sync.not()).ignored()));

    first.then(rest.repeated()).to(fallback)
}
