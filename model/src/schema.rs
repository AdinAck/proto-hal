//! The model components related to [Schemas](Schema).
//!
//! A schema is the set of [variants](crate::variant) exposed by a [field](crate::field). Fields may share schemas, or
//! use an anonymous inherently derived schema.
//!
//! A schema may hold asymmetrical variants, the compatibility of which is determined by the consuming field's
//! [access](crate::access) modality.
//!
//! *Note: [Access modality](crate::access) is a property of [fields](crate::field).*

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaId(pub(crate) usize);

/// A [`Schema`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct SchemaNode {
    pub(crate) schema: Schema,
    pub(crate) variants: Vec<VariantId>,
    pub(crate) read_variants: Vec<VariantId>,
    pub(crate) write_variants: Vec<VariantId>,
}
