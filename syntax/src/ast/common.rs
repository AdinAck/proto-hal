//! Constructs shared by multiple definition kinds: heads, paths, positions,
//! array machinery, access modalities, and reset values.

use super::Spanned;

/// The common part of every definition's head: the template it derives from,
/// its name, and its array designators.
///
/// Written as `#template`, `#template as name`, or `name` — where array
/// definitions may suffix the name with [`Indices`].
///
/// Every part is optional at the syntax level: an invocation without a name
/// takes its template's, and requiredness is judged at elaboration after
/// template merging.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Head<'src> {
    /// `#path` — the definition this one inherits from.
    pub template: Option<Spanned<Path<'src>>>,
    pub name: Option<Spanned<&'src str>>,
    /// `[a, b, c]` or `[0..=15]` — array element designators.
    pub indices: Option<Spanned<Indices<'src>>>,
}

/// A `.`-separated reference to a definition, schema, or variant.
#[derive(Debug, Clone, PartialEq)]
pub struct Path<'src> {
    pub segments: Vec<Spanned<&'src str>>,
}

/// Array element designators, determining both the element count and each
/// element's name suffix.
#[derive(Debug, Clone, PartialEq)]
pub enum Indices<'src> {
    /// `[a, b, c]` — each element is suffixed with a name.
    Names(Vec<Spanned<&'src str>>),
    /// `[0..=15]` — each element is suffixed with an index.
    Range(NumRange),
    /// `[0..=31, ...]` — as [`Range`](Indices::Range), continuing across the
    /// elements of the enclosing array: each subsequent element's indices
    /// pick up where the previous left off.
    Series(NumRange),
}

/// A single access modality word.
///
/// Modalities compose as a *set*: `read write` (in either order) expresses
/// what the model calls read-write access. The grammar admits each word at
/// most once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Store,
    VolatileStore,
}

/// `0..2` or `0..=1` — a numeric range with Rust semantics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumRange {
    pub start: Spanned<u32>,
    pub end: Spanned<u32>,
    /// `..=` rather than `..`.
    pub inclusive: bool,
    /// The optional step size for the range (default 1).
    pub step: Option<Spanned<u32>>,
}

/// `@ ...` — the position a definition occupies within its parent.
#[derive(Debug, Clone, PartialEq)]
pub enum Domain {
    /// `@ 0xc4` — an address, offset, or single-bit position.
    Value(u32),
    /// `@ 0..=1` — a bit domain.
    Range(NumRange),
    /// `@ [0x0, 0x4]` or `@ [0..=1, ...]` — one entry per array element.
    List(Vec<Spanned<ListEntry>>), // TODO: why is this not inherent from lists in general? shouldn't domains be
                                   //       optionally lists like other values?
}

/// One element of a [`Domain::List`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListEntry {
    Value(u32),
    Range(NumRange),
    Rest(Rest),
}

/// `...` — the remaining elements continue the pattern.
///
/// Without a [stride](Rest::stride), the step is positional: field domains
/// pack adjacently by width, register offsets step by the register size, and
/// variant values increment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rest {
    /// `+0x4` / `-0x4` — overrides the default step.
    pub stride: Option<Spanned<Stride>>,
}

/// An explicit step for [`Rest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stride {
    /// `-` rather than `+`.
    pub negative: bool,
    pub magnitude: u32,
}

/// `read variant` / `write variant` — which numericity of a `read write`
/// container a variant occupies. Plain variants occupy both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Read,
    Write,
}

/// `reset ...` — a reset value, at the register or field level.
///
/// Where both levels specify one, they must agree: redundancy is permitted;
/// contradiction never is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetValue<'src> {
    /// `reset 0x7f`
    Value(u32),
    /// `reset Disabled` — by variant name (fields only).
    Variant(&'src str),
}
