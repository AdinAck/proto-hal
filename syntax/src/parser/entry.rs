//! Parsing of [entries](Entry).

use chumsky::prelude::*;

use super::{
    Error, Input,
    iteration::{list_of, range},
    set::set_of,
    value::value,
};
use crate::{ast::Entry, span::Spanned};

/// An [`Entry`].
///
/// ```text
/// entry ::= range | value | list | set
/// ```
pub(super) fn entry<'src>()
-> impl Parser<'src, Input<'src>, Spanned<Entry<'src>>, Error<'src>> + Clone {
    recursive(|entry| {
        choice((
            range().map(Entry::Range),
            value().map(Entry::Value),
            list_of(entry.clone()).map(Entry::List),
            set_of(entry).map(Entry::Set),
        ))
        .spanned()
    })
}
