//! Schemas: reusable, shareable variant sets.
//!
//! A schema is *physical*: its codegen manifestation is the module where the
//! variant state types are written. A schema definition is noncorporeal like
//! any other top-level definition — placing it inside the device tree (as a
//! [`DeviceItem`](super::DeviceItem), [`PeripheralItem`](super::PeripheralItem),
//! [`RegisterItem`](super::RegisterItem), or group member) gives it the
//! location those types manifest at.

use super::{FieldItem, Head, Span, Spanned};

/// `schema name { ... }`
///
/// Schemas have no access modality: they are variant vocabularies — plain,
/// `read`, or `write` variants — compatible with any field whose modality
/// admits every side they name.
#[derive(Debug, Clone, PartialEq)]
pub struct Schema<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    /// `leaky` — all interactions with fields of this schema are `unsafe`.
    pub leaky: Option<Span>,
    pub head: Head<'src>,
    pub body: Option<Spanned<Vec<Spanned<SchemaItem<'src>>>>>,
}

/// Schema bodies hold variants, exactly as field bodies do.
pub type SchemaItem<'src> = FieldItem<'src>;
