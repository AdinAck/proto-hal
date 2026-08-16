//! Syntactic analysis: [`Token`]s → [`File`].
//!
//! Each definition kind has its own grammar: symbols are contextual operators,
//! so `~` parses only after a variant's name, `reset` only on registers and
//! fields, and so on. Containers likewise parse only the item kinds that may
//! appear within them. The containment hierarchy is strictly top-down, so no
//! parser (except the token-level [block skipper](recovery::block)) is
//! recursive.
//!
//! Head properties follow a canonical order: domain, schema reference, reset,
//! `requires` clauses (plain, `write`, `hardware write`), then the body.
//!
//! # Error recovery
//!
//! The parser reports *all* the errors it can. Recovery operates at two
//! levels: an unparseable region between items is consumed up to the next
//! plausible item (leaving an `Error` node behind), and a structurally broken
//! body is discarded whole. See [`recovery`].

mod atom;
mod def;
mod prop;
mod recovery;

use chumsky::{input::ValueInput, prelude::*};

use crate::{
    ast::{File, FileItem, Span},
    token::Token,
};

/// The parser error configuration: rich errors over [`Token`]s.
pub(crate) type Extra<'tokens, 'src> = extra::Err<Rich<'tokens, Token<'src>, Span>>;

/// Shorthand for the input every parser here operates on: a stream of
/// [`Token`]s carrying byte spans.
pub trait TokenInput<'tokens, 'src: 'tokens>:
    ValueInput<'tokens, Token = Token<'src>, Span = Span>
{
}

impl<'tokens, 'src: 'tokens, I> TokenInput<'tokens, 'src> for I where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>
{
}

/// The complete grammar: a sequence of top-level items.
pub fn file<'tokens, 'src: 'tokens, I>() -> impl Parser<'tokens, I, File<'src>, Extra<'tokens, 'src>>
where
    I: TokenInput<'tokens, 'src>,
{
    let item = choice((
        def::import().map(FileItem::Import),
        def::device().map(FileItem::Device),
        def::peripheral_group().map(FileItem::PeripheralGroup),
        def::peripheral().map(FileItem::Peripheral),
        def::register_group().map(FileItem::RegisterGroup),
        def::register().map(FileItem::Register),
        def::field_group().map(FileItem::FieldGroup),
        def::field().map(FileItem::Field),
        def::schema().map(FileItem::Schema),
        def::variant().map(FileItem::Variant),
    ))
    .recover_with(via_parser(recovery::garbage(FileItem::Error)));

    item.spanned()
        .repeated()
        .collect()
        .map(|items| File { items })
        .then_ignore(end())
}
