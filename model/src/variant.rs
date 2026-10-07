//! The model components related to [Variants](Variant).
//!
//! A variant is a named value of the data read from or written to a [field](crate::field), as defined by a
//! [schema](crate::schema).
//!
//! # Access specificity
//!
//! A variant is *access-agnostic* when it participates in both read and write accesses of a field. A variant is
//! *access-specific* when it participates in *either* read *or* write accesses of a field, namely read variants and
//! write variants.
//!
//! A variant is *readable* when it participates in read accesses of a field, namely access-agnostic and read
//! variants. A variant is *writable* when it participates in write accesses of a field, namely access-agnostic and
//! write variants.
//!
//! # Inert
//!
//! A variant is *inert* when writing it is considered a no-op. For register write operations, fields with any inert
//! variants need not be specified as an inert variant will be selected by default.
//!
//! Read variants participate only in read accesses, and as such *cannot* be inert.

use derive_more::{Display, From, Into};
use source::{Span, Spanned};
use ters::ters;

use crate::{Word, schema::SchemaId};

/// A variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// The variant name.
    pub name: Spanned<String>,
    /// The value the variant is represented by.
    pub discriminant: Spanned<Word>,
    /// The [inert](crate::variant#inert) qualifier, if applicable.
    pub inert: Option<Spanned<Inert>>,
    /// The [access qualifier](AccessQualifier), if applicable.
    pub access: Option<Spanned<AccessQualifier>>,
    /// Doc comments pertaining to this variant.
    pub docs: Vec<String>,
    /// The span of the declaration.
    pub span: Span,
}

/// A [`Variant`] node in the model.
#[ters]
#[derive(Debug, Clone)]
pub(crate) struct VariantNode {
    pub(crate) variant: Variant,
    /// The schema defining the variant.
    #[get(deref)]
    schema: SchemaId,
}

impl VariantNode {
    /// Create a node containing the provided variant, defined by the provided schema.
    pub(crate) fn new(variant: Variant, schema: SchemaId) -> Self {
        Self { variant, schema }
    }
}

/// A handle to a [`Variant`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct VariantId(usize);

/// The [inert](crate::variant#inert) qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Inert;

/// An access qualifier, making a variant [access-specific](crate::variant#access-specificity). Displays in lowercase,
/// such as `write`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[display(rename_all = "lowercase")]
pub enum AccessQualifier {
    /// Makes the variant a read variant.
    Read,
    /// Makes the variant a write variant.
    Write,
}
