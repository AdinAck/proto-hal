//! The model components related to [Variants](Variant).
//!
//! A variant is a named value of the data read from or written to a [field](crate::field), as defined by a
//! [schema](crate::schema).
//!
//! # Inert
//!
//! A variant is *inert* when writing it is considered a no-op. For register write operations, fields with any inert
//! variants need not be specified as an inert variant will be selected by default.

use derive_more::{From, Into};
use source::{Span, Spanned};

use crate::{Word, schema::SchemaId};

/// A variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The variant name.
    pub name: Spanned<String>,
    /// The value the variant is represented by.
    pub discriminant: Spanned<Word>,
    /// Whether the variant is [inert](crate::variant#inert).
    pub inert: bool,
    /// Doc comments pertaining to this variant.
    pub docs: Vec<String>,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Variant`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct VariantId(usize);

/// A [`Variant`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct VariantNode {
    pub(crate) variant: Variant,
    pub(crate) schema: SchemaId,
}

impl VariantNode {
    /// Create a node containing the provided variant, defined by the provided schema.
    pub(crate) fn new(variant: Variant, schema: SchemaId) -> Self {
        Self { variant, schema }
    }
}
