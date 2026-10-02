//! The abstract syntax tree.
//!
//! AST nodes implement [`Display`](fmt::Display) rendered as source text in canonical form.

use std::fmt;

use derive_more::Display;
use itertools::Itertools;

use crate::{
    span::Spanned,
    token::{Ident, Literal},
};

/// A source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File;

/// A single value i.e. `0x14`, `foo`, `15:0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Value<'src> {
    /// A [`Literal`].
    Literal(Literal),
    /// An [`Ident`].
    Ident(Ident<'src>),
    /// Inclusive bits, most significant first i.e. `15:0`.
    #[display("{msb}:{lsb}")]
    Domain {
        /// The most significant bit.
        msb: Literal,
        /// The least significant bit.
        lsb: Literal,
    },
}

/// An entry of a [list](List) or a [set](Set) i.e. `0..=7`, `foo`, `[0, 2..=7]`.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Entry<'src> {
    /// A [`Range`].
    Range(Range),
    /// A [`Value`].
    Value(Value<'src>),
    /// A nested [`List`].
    List(List<'src>),
    /// A [`Set`].
    Set(Set<'src>),
}

/// An inclusive or exclusive range between two [`Literal`]s with an optional step i.e. `0..16`, `4..=60 by 4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// The first literal.
    pub start: Literal,
    /// The literal the range ends at.
    pub end: Literal,
    /// Whether [`end`](Range::end) is included (`..=`) or not (`..`).
    pub inclusive: bool,
    /// The distance between successive literals, written with `by`.
    pub step: Option<i64>,
}

/// A bracketed list i.e. `[a, b]`, `[0x8, ... by 0x14]`. (*See [`Entry`] and [`Progression`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List<'src> {
    /// The entries, in order.
    pub entries: Vec<Spanned<Entry<'src>>>,
    /// The [`Progression`] continuing the entries, if any.
    pub progression: Option<Spanned<Progression<'src>>>,
}

/// A [list](List)'s last entry, `...`, continuing the entry before it i.e. `... by 4`, `... in #foo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progression<'src> {
    /// The distance between successive values or sequences, written with `by`.
    pub step: Option<i64>,
    /// The iteration scope, named by its variable.
    pub scope: Option<Spanned<Ident<'src>>>,
}

/// A braced set i.e. `{A, B}`. (*See [`Entry`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set<'src> {
    /// The entries.
    pub entries: Vec<Spanned<Entry<'src>>>,
}

/// A name i.e. `foo`, `foo[a, b, c]`, `foo[1..=3]bar`. (*See [`Fragment`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name<'src> {
    /// The fragments, in order.
    pub fragments: Vec<Spanned<Fragment<'src>>>,
}

/// A fragment of a [`Name`] i.e. `foo`, `[a, b, c]`, `{1, 2}`, `[#foo]`.
#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub enum Fragment<'src> {
    /// Text, an [`Ident`].
    Text(Ident<'src>),
    /// A [`List`].
    List(List<'src>),
    /// A [`Set`].
    Set(Set<'src>),
    /// An iteration variable, inserted i.e. `[#foo]`.
    #[display("[#{_0}]")]
    Insertion(Ident<'src>),
}

/// A path i.e. `foo.bar`, `device.foo.Bar`, `foo{1, 2}.bar.{Baz, Qux}`. (*See [`Name`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<'src> {
    /// Whether the path starts at the device i.e. `device.foo`.
    pub rooted: bool,
    /// The path segments, in order.
    pub segments: Vec<Spanned<Name<'src>>>,
}

/// A requirement i.e. `foo.Bar & baz.Qux | foo.Baz`. (*See [`Pattern`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement<'src> {
    /// The [patterns](Pattern) specified in the requirement.
    pub patterns: Vec<Spanned<Pattern<'src>>>,
}

/// A pattern of a [`Requirement`] i.e. `foo.Bar & baz.Qux`. (*See [`Path`]*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern<'src> {
    /// The [paths](Path) specified in the pattern.
    pub paths: Vec<Spanned<Path<'src>>>,
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = if self.inclusive { "..=" } else { ".." };
        write!(f, "{}{op}{}", self.start, self.end)?;

        if let Some(step) = self.step {
            write!(f, " by {step}")?;
        }

        Ok(())
    }
}

impl fmt::Display for List<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}",
            self.entries.iter().map(|entry| &entry.inner).format(", ")
        )?;

        if let Some(progression) = &self.progression {
            write!(f, ", {}", progression.inner)?;
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
            write!(f, " in #{}", scope.inner)?;
        }

        Ok(())
    }
}

impl fmt::Display for Set<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}}}",
            self.entries.iter().map(|entry| &entry.inner).format(", ")
        )
    }
}

impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.fragments
                .iter()
                .map(|fragment| &fragment.inner)
                .format("")
        )
    }
}

impl fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.rooted {
            write!(f, "device.")?;
        }

        write!(
            f,
            "{}",
            self.segments
                .iter()
                .map(|segment| &segment.inner)
                .format(".")
        )
    }
}

impl fmt::Display for Requirement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.patterns
                .iter()
                .map(|pattern| &pattern.inner)
                .format(" | ")
        )
    }
}

impl fmt::Display for Pattern<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.paths.iter().map(|path| &path.inner).format(" & ")
        )
    }
}
