//! [Sets](Set) and their [entries](Entry).

use std::fmt;

use derive_more::Display;
use itertools::Itertools;

use super::{Range, Value};
use crate::util::Spanned;

/// A braced set, such as `{A, B}` or `{1, 3..=5}`. (*See [`Entry`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set<'src> {
    /// The entries.
    pub entries: Vec<Spanned<Entry<'src>>>,
}

/// An entry of a [`Set`], such as `A` or `3..=5`.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Entry<'src> {
    /// A [`Range`].
    Range(Range<'src>),
    /// A [`Value`].
    Value(Value<'src>),
}

impl fmt::Display for Set<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}", self.entries.iter().format(", "))
    }
}
