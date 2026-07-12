//! Peripherals: the top level of the hardware hierarchy.

use super::{Domain, Head, Register, RegisterGroup, Schema, Space, Span, Spanned};

/// `peripheral name @ base { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct Peripheral<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    /// `leaky` — all interactions with the peripheral's fields are `unsafe`.
    pub leaky: Option<Span>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the base address(es). Peripheral arrays must list every address
    /// explicitly: hardware regions bear no relation to register sizes, so
    /// [`Rest`](super::ListEntry::Rest) is rejected at elaboration.
    pub domain: Option<Spanned<Domain>>,
    /// Ontological entitlements: the peripheral exists only when satisfied.
    pub requires: Option<Spanned<Space<'src>>>,
    pub body: Option<Spanned<Vec<Spanned<PeripheralItem<'src>>>>>,
}

/// What may appear within a peripheral.
#[derive(Debug, Clone, PartialEq)]
pub enum PeripheralItem<'src> {
    Register(Register<'src>),
    RegisterGroup(RegisterGroup<'src>),
    /// A schema placement: the schema's types manifest within this peripheral.
    Schema(Schema<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}
