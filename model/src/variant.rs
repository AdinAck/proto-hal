//! The model components related to [Variants](Variant).
//!
//! A variant is a named value of the data read from or written to a [field](crate::field), as defined by a
//! [schema](crate::schema).
//!
//! # Inert
//!
//! A variant is *inert* when writing it is considered a no-op. For register write operations, fields with any inert
//! variants need not be specified as an inert variant will be selected by default.

use source::{Span, Spanned};

use crate::schema::SchemaId;

/// A variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The variant name.
    pub name: Spanned<String>,
    /// The value the variant is represented by.
    pub discriminant: Spanned<u32>,
    /// Whether the variant is [inert](crate::variant#inert).
    pub inert: bool,
    /// Doc comments pertaining to this variant.
    pub docs: Vec<String>,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Variant`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariantId(pub(crate) usize);

/// A [`Variant`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct VariantNode {
    pub(crate) variant: Variant,
    pub(crate) schema: SchemaId,
}
