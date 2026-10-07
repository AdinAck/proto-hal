//! The model components related to [Fields](Field).
//!
//! A field [domain](Domain) describes the contiguous bit region within its parent [register](crate::register) that it
//! occupies.
//!
//! A field has an [access modality](crate::access), and may expose the variants of a [schema](crate::schema),
//! potentially common to multiple fields. A field without a schema is [numeric](self#numericity).
//!
//! A field may define a [reset](Reset) value, either as a numeric value or one of its variants.
//!
//! # Numericity
//!
//! A field is *numeric* when it has no schema, meaning the values read or written to it have no additional semantics
//! beyond a numeric value. A field is *enumerated* when the values read or written to it have non-numeric semantics,
//! represented as [variants](crate::variant) defined by the field's schema.
//!
//! *Note: Numeric fields imply that *any* value which fits in the field is valid.*

use source::{Span, Spanned};

use crate::{access::Access, register::RegisterId, schema::SchemaId, variant::VariantId};

/// A field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The field name.
    pub name: Spanned<String>,
    /// The bits of the register the field occupies.
    pub domain: Spanned<Domain>,
    /// The field access modality.
    pub access: Spanned<Access>,
    /// The schema of the field variants, if applicable, spanning where the field takes it from.
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

/// A field domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Domain {
    msb: u8,
    lsb: u8,
}

impl Domain {
    /// Create a domain, if `msb` is not less than `lsb`.
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
}

/// A field reset value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reset {
    /// A value.
    Value(u32),
    /// A variant.
    Variant(VariantId),
}

/// A handle to a [`Field`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub(crate) usize);

/// A [`Field`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct FieldNode {
    pub(crate) field: Field,
    pub(crate) register: RegisterId,
}
