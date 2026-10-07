//! The model components related to [Fields](Field).
//!
//! A field [domain](Domain) describes the contiguous bit region within its parent [register](crate::register) that it
//! occupies.
//!
//! A field has an [access modality](access_modality), and may expose the variants of a [schema](crate::schema),
//! potentially common to multiple fields. A field without a schema is [numeric](self#numericity).
//!
//! A field may define a [reset](Reset) value, either as a numeric value or one of its variants.
//!
//! # Schema
//!
//! A field's variants are defined by a *schema*. A field without a schema is [numeric](self#numericity). Otherwise,
//! the field either *assumes* a schema, sharing it with the other fields that assume it, or defines *inherent*
//! variants, forming a schema unique to the field. A field may also *refine* schemas, composing their variants into the
//! field's inherent schema.
// TODO: expand into a full explanation of the relationship between fields and schemas
//!
//! # Numericity
//!
//! A field is *numeric* when it has no schema, meaning the values read or written to it have no additional semantics
//! beyond a numeric value. A field is *enumerated* when the values read or written to it have non-numeric semantics,
//! represented as [variants](crate::variant) defined by the field's schema.
//!
//! *Note: Numeric fields imply that *any* value which fits in the field is valid.*

pub mod access_modality;

use derive_more::{Display, From, Into};
use source::{Span, Spanned};

use self::access_modality::AccessModality;
use crate::{Word, register::RegisterId, schema::SchemaId, variant::VariantId};

/// A field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The field name.
    pub name: Spanned<String>,
    /// The bits of the register the field occupies.
    pub domain: Spanned<Domain>,
    /// The field [access modality](access_modality).
    pub access_modality: Spanned<AccessModality>,
    /// The [schema](crate::field#schema) defining the field's variants, if applicable.
    pub schema: Option<Spanned<SchemaId>>,
    /// The reset value, if applicable.
    pub reset: Option<Spanned<Reset>>,
    /// Doc comments pertaining to this field.
    pub docs: Vec<String>,
    /// Whether the field is [leaky](crate#leaky).
    pub leaky: bool,
    /// The span of the declaration.
    pub span: Span,
}

/// A field domain. Displays as its bits, such as `15:0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[display("{msb}:{lsb}")]
pub struct Domain {
    msb: u8,
    lsb: u8,
}

impl Domain {
    /// Create a domain from the provided most and least significant bits, if they are in order.
    pub fn new(msb: u8, lsb: u8) -> Option<Self> {
        (msb >= lsb).then_some(Self { msb, lsb })
    }

    /// The most significant bit.
    pub fn msb(self) -> u8 {
        self.msb
    }

    /// The least significant bit.
    pub fn lsb(self) -> u8 {
        self.lsb
    }

    /// The number of bits.
    pub fn width(self) -> u8 {
        self.msb - self.lsb + 1
    }

    /// Determine whether the provided domain resides entirely within this domain or not.
    pub fn contains(self, other: Self) -> bool {
        self.msb >= other.msb && self.lsb <= other.lsb
    }
}

/// A field reset value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reset {
    /// A value.
    Value(Word),
    /// A variant.
    Variant(VariantId),
}

/// A handle to a [`Field`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct FieldId(usize);

/// A [`Field`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct FieldNode {
    pub(crate) field: Field,
    pub(crate) register: RegisterId,
}

impl FieldNode {
    /// Create a node containing the provided field as a child of the provided register.
    pub(crate) fn new(field: Field, register: RegisterId) -> Self {
        Self { field, register }
    }
}
