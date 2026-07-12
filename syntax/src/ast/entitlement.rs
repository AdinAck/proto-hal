//! The requirements expressed by `requires` clauses.
//!
//! Entitlements are *not* a boolean expression tree — mirroring
//! `phm::entitlement`, a requirement is a list of lists. The `&` and `|`
//! symbols are for speech, not structure: the grammar only admits disjunctive
//! form, so `a & (b | c)` is a syntax error rather than something to normalize.

use super::{NumRange, Spanned};

/// A space of [`Pattern`]s, *any* of which satisfies the requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct Space<'src> {
    /// `|`-separated.
    pub patterns: Vec<Spanned<Pattern<'src>>>,
}

/// A set of entitlements, *all* of which must be satisfied — one per
/// distinct field.
///
/// May be parenthesized: `(a.b & c.d)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern<'src> {
    /// `&`-separated entitled fields.
    pub entitlements: Vec<Spanned<Entitled<'src>>>,
}

/// One entitled field of a pattern.
///
/// The field inhabits the named variant — or, with a set, *any* of them:
/// `cordic.csr.scale.{N0, N1, N2}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Entitled<'src> {
    /// The path — ending at the variant, or at the field when a set names
    /// the variants.
    pub segments: Vec<Spanned<Segment<'src>>>,
    /// `.{A, B}` — the variant set. Braces, not brackets: variant membership
    /// is unordered, where bracketed lists zip in parallel.
    pub set: Option<Vec<Spanned<&'src str>>>,
}

/// A segment of an entitlement path: a name — or an array correspondence,
/// `ccr[0..8]`, naming one target per element of the enclosing array.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment<'src> {
    pub name: &'src str,
    /// `[0..8]` — the corresponding target indices, zipped by ordinal
    /// against the enclosing array's elements.
    pub elements: Option<NumRange>,
}
