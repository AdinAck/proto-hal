//! The model components related to [Schemas](Schema).
//!
//! A schema is the set of [variants](crate::variant) exposed by a [field](crate::field). Fields may share schemas, or
//! use an anonymous inherently derived schema.
//!
//! # Access spaces
//!
//! An *access space* is a space of discriminants, occupied by the variants of a schema that participate in one access.
//! A variant occupies the discriminant assigned to it. [Readable](crate::variant#access-specificity) variants occupy
//! the *read space*. Writable variants occupy the *write space*. A *read discriminant* is a discriminant within the
//! read space. A *write discriminant* is a discriminant within the write space.
//!
//! # Consistency
//!
//! Variants *overlap* when they occupy the same discriminant within an [access space](#access-spaces). A schema is
//! *consistent* when the schema defines no overlapping variants. Unlike names, which are unique within a schema, a
//! discriminant may be assigned to both a read variant and a write variant.

use derive_more::{Display, From, Into};
use source::{Span, Spanned};
use ters::ters;

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

/// A [`Schema`] node in the model.
#[ters]
#[derive(Debug, Clone)]
pub(crate) struct SchemaNode {
    pub(crate) schema: Schema,
    /// The variants defined by the schema, in the order they were added.
    #[get]
    variants: Vec<VariantId>,
}

impl SchemaNode {
    /// Create a node containing the provided schema, with no variants.
    pub(crate) fn new(schema: Schema) -> Self {
        Self {
            schema,
            variants: Vec::new(),
        }
    }

    /// Add the provided variant to the variants defined by the schema.
    pub(crate) fn add_variant(&mut self, variant: VariantId) {
        self.variants.push(variant);
    }
}

/// A handle to a [`Schema`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct SchemaId(usize);

/// An [access space](crate::schema#access-spaces) of a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[display(rename_all = "lowercase")]
pub(crate) enum AccessSpace {
    /// The read space.
    Read,
    /// The write space.
    Write,
}
