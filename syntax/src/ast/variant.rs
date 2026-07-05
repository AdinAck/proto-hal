//! Variants: the states a field may occupy.

use super::{Head, Side, Space, Span, Spanned};

/// `variant Name ~ value`
#[derive(Debug, Clone, PartialEq)]
pub struct Variant<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    /// `leaky` — interactions with this variant are `unsafe`.
    pub leaky: Option<Span>,
    /// `inert` — writing this variant has no effect on hardware.
    pub inert: Option<Span>,
    /// `read variant` / `write variant` — the numericity this variant
    /// occupies within a `read write` container; plain variants occupy both.
    pub side: Option<Spanned<Side>>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `~` — the value(s) the variant occupies.
    pub value: Option<Spanned<VariantValue>>,
    /// Statewise entitlements: the variant may be inhabited only when
    /// satisfied.
    pub requires: Option<Spanned<Space<'src>>>,
}

/// The value(s) a variant occupies.
///
/// Unlike [`Domain`](super::Domain), variant values are scalar — ranges are
/// unrepresentable here by design.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantValue {
    /// `~ 0x3`
    Value(u32),
    /// `~ [0x0, 0x4, ...]` — one entry per array element.
    List(Vec<Spanned<ValueEntry>>),
}

/// One element of a [`VariantValue::List`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueEntry {
    Value(u32),
    Rest(super::Rest),
}
