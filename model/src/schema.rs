//! The model components related to [Schemas](Schema).
//!
//! A schema is the set of [variants](crate::variant) exposed by a [field](crate::field). Fields may share schemas, or
//! use an anonymous inherently derived schema.
//!
//! # Access specificity
//!
//! A variant is *access-agnostic* when it participates in both read and write accesses of a field. A variant is
//! *access-specific* when it participates in *either* read *or* write accesses of a field, namely read variants and
//! write variants.

use derive_more::{From, Into};
use source::{Span, Spanned};

use crate::variant::VariantId;

/// A schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    /// The schema name, if applicable.
    ///
    /// *Note: Schemas inherent to a field have no name.*
    pub name: Option<Spanned<String>>,
    /// Doc comments pertaining to this schema.
    pub docs: Vec<String>,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Schema`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct SchemaId(usize);

/// A [`Schema`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct SchemaNode {
    pub(crate) schema: Schema,
    pub(crate) agnostic_variants: Vec<VariantId>,
    pub(crate) read_variants: Vec<VariantId>,
    pub(crate) write_variants: Vec<VariantId>,
}

impl SchemaNode {
    /// Create a node containing the provided schema, with no variants.
    pub(crate) fn new(schema: Schema) -> Self {
        Self {
            schema,
            agnostic_variants: Vec::new(),
            read_variants: Vec::new(),
            write_variants: Vec::new(),
        }
    }
}
