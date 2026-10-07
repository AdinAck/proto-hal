//! The model components related to [Fields](Field).
//!
//! A field [domain](Domain) describes the contiguous bit region within its parent [register](crate::register) that it
//! occupies.
//!
//! A field has an [access modality](access_modality), and may expose the variants of a [schema](crate::schema),
//! potentially common to multiple fields. A field without a schema is [numeric](self#numericity).
//!
//! A field may define a [reset](self#resets).
//!
//! # Schema
//!
//! A field's variants are defined by a *schema*. A field without a schema is [numeric](self#numericity). Otherwise,
//! the field either *assumes* a schema, sharing it with the other fields that assume it, or defines *inherent*
//! variants, forming a schema unique to the field. A field may also *refine* schemas, composing their variants into the
//! field's inherent schema.
// TODO: expand into a full explanation of the relationship between fields and schemas
//!
//! # Compatibility
//!
//! A variant *implies* a particular access when any property of the variant pertains to that access. For example,
//! variants that participate in *reads* (read variants) necessarily implies that the field *must* be readable.
//! Conversely, a *write* variant implies that the field must be *writable*, as does an [inert](crate::variant#inert)
//! variant, since the property of inertness solely pertains to when the variant is *written*.
//!
//! The variants a field *cannot expose* is determined by the field's [access modality](access_modality):
//!
//! | Access modality          | Variants the field cannot expose   |
//! | ------------------------ | ---------------------------------- |
//! | `Read`                   | variants implying write access     |
//! | `Write`                  | variants implying read access      |
//! | `ReadWrite`              | none                               |
//! | `Store`, `VolatileStore` | access-specific and inert variants |
//!
//! Fields with modality `Store` or `VolatileStore` cannot expose access-specific variants, as the data read from and
//! written to such a field are defined as having the same semantics. These fields also cannot expose
//! [inert](crate::variant#inert) variants, as inertness may only apply to ephemeral write operations. (*See [access
//! modalities](access_modality)*)
//!
//! A field is *compatible* with a schema when the field can expose all variants the schema defines.
//!
//! A field *supports* read and/or write access contingent on the allowance of the respective [access
//! modality](access_modality). A field supporting a particular access (read or write) should expose variants
//! participating in that access, as the access would otherwise go unused.
//!
//! # Numericity
//!
//! A field is *numeric* when it has no schema, meaning the values read or written to it have no additional semantics
//! beyond a numeric value. A field is *enumerated* when the values read or written to it have non-numeric semantics,
//! represented as [variants](crate::variant) defined by the field's schema.
//!
//! *Note: Numeric fields imply that *any* value representable by the field is valid.*
//!
//! # Resets
//!
//! A field's *reset* is the value, or variant, that the field initially inhabits.
//!
//! A field is a *store field* when the field has modality [`Store`](AccessModality::Store) or
//! [`VolatileStore`](AccessModality::VolatileStore). Only store fields define resets, as store fields are
//! [resolvable](access_modality), [`VolatileStore`](AccessModality::VolatileStore) fields conditionally so.
//!
//! A register's reset is derived from the resets of the store fields the register hosts, with all other bits being
//! zero. (*See [`Model::reset`](crate::Model::reset)*)

pub mod access_modality;

use derive_more::{Display, From, Into};
use source::{Span, Spanned};

use self::access_modality::AccessModality;
use crate::{Leaky, Word, register::RegisterId, schema::SchemaId, variant::VariantId};

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
    /// The field [reset](crate::field#resets), if applicable.
    pub reset: Option<Spanned<Reset>>,
    /// Doc comments pertaining to this field.
    pub docs: Vec<String>,
    /// The [leaky](crate#leaky) qualifier, if applicable.
    pub leaky: Option<Spanned<Leaky>>,
    /// The span of the declaration.
    pub span: Span,
}

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

/// A handle to a [`Field`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct FieldId(usize);

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
    pub fn width(self) -> u16 {
        u16::from(self.msb - self.lsb) + 1
    }

    /// The maximum value representable by this domain. (2^width - 1)
    pub fn max_value(self) -> Word {
        // invariant: a domain is at least one bit wide, so the shift is less than `Word::BITS`
        Word::MAX >> Word::BITS.saturating_sub(self.width().into())
    }

    /// Determine whether the provided domain resides entirely within this domain or not.
    pub fn contains(self, other: Self) -> bool {
        self.msb >= other.msb && self.lsb <= other.lsb
    }
}

/// A field [reset](self#resets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reset {
    /// A value.
    Value(Word),
    /// A variant.
    Variant(VariantId),
}
