//! [Lists](List) and their [entries](Entry).

use std::fmt;

use derive_more::Display;
use itertools::Itertools;

use super::{Range, Set, Step, Value};
use crate::{
    token::Ident,
    util::{Parsed, Spanned},
};

/// A bracketed list i.e. `[a, b]`, `[0x8, ... by 0x14]`. (*See [`Entry`] and [`Progression`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List<'src> {
    /// The entries, in order.
    pub entries: Vec<Spanned<Parsed<Entry<'src>>>>,
    /// The [`Progression`] continuing the entries, if any.
    pub progression: Option<Spanned<Progression<'src>>>,
}

/// An entry of a [`List`] i.e. `0..=7`, `foo`, `{1, 2}`, and `[0, 2..=7]` in `[[0, 2..=7], ...]`.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Entry<'src> {
    /// A [`Range`].
    Range(Range<'src>),
    /// A [`Value`].
    Value(Value<'src>),
    /// A nested [`List`].
    List(List<'src>),
    /// A [`Set`].
    Set(Set<'src>),
}

/// A [list](List)'s last entry, `...`, continuing the entry before it i.e. `... by 4`, `... in #foo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progression<'src> {
    /// The distance between successive values or sequences, written with `by`.
    pub step: Option<Step<'src>>,
    /// The iteration scope, named by its variable.
    pub scope: Option<Spanned<Ident<'src>>>,
}

impl fmt::Display for List<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}", self.entries.iter().format(", "))?;

        if let Some(progression) = &self.progression {
            write!(f, ", {progression}")?;
        }

        write!(f, "]")
    }
}

impl fmt::Display for Progression<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "...")?;

        if let Some(step) = self.step {
            write!(f, " by {step}")?;
        }

        if let Some(scope) = &self.scope {
            write!(f, " in #{scope}")?;
        }

        Ok(())
    }
}
