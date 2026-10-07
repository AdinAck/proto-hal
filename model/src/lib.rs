//! The MMIO device model.
//!
//! # Items
//!
//! An *item* is a [peripheral], [register], [field], [schema], or [variant] of the device. In the device model, each
//! item is a [node](Node), referred to by its handle.
//!
//! An item is *memory-mapped* when it occupies a [domain](#domain), as peripherals, registers, and fields do.
//!
//! ## Leaky
//!
//! An [item](#items) is *leaky* when the model cannot encapsulate its hardware invariants or its description is
//! knowingly incomplete. Using a leaky item may introduce unsoundness, and any interface generated from the model will
//! reflect as such (require `unsafe`).
//!
//! # Word size
//!
//! A model's *word size*, in bytes, is the size of a register and of an address, and the alignment of addresses and
//! offsets. The rules of placement follow from it.
//!
//! *Note: A word size is from 1 to 8 bytes, as an address is at most one [`Word`].*
//!
//! # Address space
//!
//! The *address space* spans the addresses from `0` to `2^(8 × word size) - 1`, those a [word](#word-size) can
//! represent. Peripherals and registers must reside within it.
//!
//! # Domain
//!
//! A *domain* is the region a peripheral, register, or field occupies. A register's domain is the [word](#word-size)
//! at its address, and a field's domain is the bits it occupies within its register's domain. A peripheral's domain
//! spans from its base address to the end of its last register's domain.

#![deny(missing_docs)]

pub mod diagnostic;
pub mod field;
pub mod peripheral;
pub mod register;
pub mod schema;
pub mod variant;

mod validation;
mod word_size;

use std::ops::{Index, IndexMut};

use derive_more::Display;
use source::Spanned;
use typed_index_collections::TiVec;

use crate::{
    field::{Field, FieldId, FieldNode},
    peripheral::{Peripheral, PeripheralId, PeripheralNode},
    register::{Register, RegisterId, RegisterNode},
    schema::{Schema, SchemaId, SchemaNode},
    variant::{Variant, VariantId, VariantNode},
};

pub use crate::word_size::WordSize;

/// The model of a device. Refer to the [crate-level docs](self) for details about the device model.
///
/// Model components can be added with the respective `add_*` methods, producing the associated handles, which are used
/// to query to model. Model components link to others with handles as well.
#[derive(Debug, Clone)]
pub struct Model {
    word_size: Spanned<Word>,
    peripherals: TiVec<PeripheralId, PeripheralNode>,
    registers: TiVec<RegisterId, RegisterNode>,
    fields: TiVec<FieldId, FieldNode>,
    schemas: TiVec<SchemaId, SchemaNode>,
    variants: TiVec<VariantId, VariantNode>,
}

impl Model {
    /// Create the model of a device with the provided [word size](crate#word-size), in bytes.
    pub fn new(word_size: Spanned<Word>) -> Self {
        Self {
            word_size,
            peripherals: TiVec::new(),
            registers: TiVec::new(),
            fields: TiVec::new(),
            schemas: TiVec::new(),
            variants: TiVec::new(),
        }
    }

    /// The [word size](crate#word-size), in bytes.
    pub fn word_size(&self) -> &Spanned<Word> {
        &self.word_size
    }

    /// Add the provided peripheral to the model, producing its handle.
    pub fn add_peripheral(&mut self, peripheral: Peripheral) -> PeripheralId {
        self.peripherals
            .push_and_get_key(PeripheralNode::new(peripheral))
    }

    /// Add the provided register to the provided peripheral, producing its handle.
    pub fn add_register(&mut self, peripheral: PeripheralId, register: Register) -> RegisterId {
        let id = self
            .registers
            .push_and_get_key(RegisterNode::new(register, peripheral));

        self.peripherals[peripheral].registers.push(id);

        id
    }

    /// Add the provided field to the provided register, producing its handle.
    pub fn add_field(&mut self, register: RegisterId, field: Field) -> FieldId {
        let id = self
            .fields
            .push_and_get_key(FieldNode::new(field, register));

        self.registers[register].fields.push(id);

        id
    }

    /// Add the provided schema to the model, producing its handle.
    pub fn add_schema(&mut self, schema: Schema) -> SchemaId {
        self.schemas.push_and_get_key(SchemaNode::new(schema))
    }

    /// Add the provided [access-agnostic](schema#access-specificity) variant to the provided schema, producing its
    /// handle.
    pub fn add_agnostic_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        let id = self
            .variants
            .push_and_get_key(VariantNode::new(variant, schema));

        self.schemas[schema].agnostic_variants.push(id);

        id
    }

    /// Add the provided read variant to the provided schema, producing its handle.
    pub fn add_read_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        let id = self
            .variants
            .push_and_get_key(VariantNode::new(variant, schema));

        self.schemas[schema].read_variants.push(id);

        id
    }

    /// Add the provided write variant to the provided schema, producing its handle.
    pub fn add_write_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        let id = self
            .variants
            .push_and_get_key(VariantNode::new(variant, schema));

        self.schemas[schema].write_variants.push(id);

        id
    }

    /// An iterator visiting all peripherals in the order they were added.
    pub fn peripherals(&self) -> impl Iterator<Item = PeripheralId> {
        self.peripherals.keys()
    }

    /// An iterator visiting all child registers of the provided peripheral in the order they were added.
    pub fn registers(&self, peripheral: PeripheralId) -> impl Iterator<Item = RegisterId> {
        self.peripherals[peripheral].registers.iter().copied()
    }

    /// An iterator visiting all child fields of the provided register in the order they were added.
    pub fn fields(&self, register: RegisterId) -> impl Iterator<Item = FieldId> {
        self.registers[register].fields.iter().copied()
    }

    /// An iterator visiting all schemas in the order they were added.
    pub fn schemas(&self) -> impl Iterator<Item = SchemaId> {
        self.schemas.keys()
    }

    /// An iterator visiting all [access-agnostic](schema#access-specificity) variants defined by the provided schema in
    /// the order they were added.
    pub fn agnostic_variants(&self, schema: SchemaId) -> impl Iterator<Item = VariantId> {
        self.schemas[schema].agnostic_variants.iter().copied()
    }

    /// An iterator visiting all [read](schema#access-specificity) variants defined by the provided schema in the order
    /// they were added.
    pub fn read_variants(&self, schema: SchemaId) -> impl Iterator<Item = VariantId> {
        self.schemas[schema].read_variants.iter().copied()
    }

    /// An iterator visiting all [write](schema#access-specificity) variants defined by the provided schema in the order
    /// they were added.
    pub fn write_variants(&self, schema: SchemaId) -> impl Iterator<Item = VariantId> {
        self.schemas[schema].write_variants.iter().copied()
    }

    /// The parent of the provided node.
    pub fn parent<C: Child>(&self, child: C) -> C::Parent {
        child.parent(self)
    }
}

/// A kind of node in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[display(rename_all = "lowercase")]
pub enum Node {
    /// A [peripheral].
    Peripheral,
    /// A [register].
    Register,
    /// A [field].
    Field,
    /// A [schema].
    Schema,
    /// A [variant].
    Variant,
}

/// A handle to a node contained by a parent node. (*See [`Model::parent`]*)
pub trait Child: Copy {
    /// The handle to the parent node.
    type Parent;

    /// The parent of the node within the provided model.
    fn parent(self, model: &Model) -> Self::Parent;
}

/// An integer representing values in the device model, such as addresses, offsets, reset values, and discriminants.
pub type Word = u64;

/// An integer twice the size of a [`Word`], representing values that may not fit in one, such as intermediate results
/// of arithmetic on words, or an address outside the permitted [address space](crate#address-space) during
/// [validation](Model::validate).
pub type WideWord = u128;

/// Implement model indexing for each node kind.
macro_rules! index {
    ($($id:ty => $nodes:ident.$node:ident: $ty:ty),* $(,)?) => {$(
        impl Index<$id> for Model {
            type Output = $ty;

            fn index(&self, id: $id) -> &$ty {
                &self.$nodes[id].$node
            }
        }

        impl IndexMut<$id> for Model {
            fn index_mut(&mut self, id: $id) -> &mut $ty {
                &mut self.$nodes[id].$node
            }
        }
    )*};
}

index! {
    PeripheralId => peripherals.peripheral: Peripheral,
    RegisterId => registers.register: Register,
    FieldId => fields.field: Field,
    SchemaId => schemas.schema: Schema,
    VariantId => variants.variant: Variant,
}

/// Implement [`Child`] for each node kind contained by a parent.
macro_rules! child {
    ($($id:ty => $nodes:ident.$parent:ident: $ty:ty),* $(,)?) => {$(
        impl Child for $id {
            type Parent = $ty;

            fn parent(self, model: &Model) -> $ty {
                model.$nodes[self].$parent
            }
        }
    )*};
}

child! {
    RegisterId => registers.peripheral: PeripheralId,
    FieldId => fields.register: RegisterId,
    VariantId => variants.schema: SchemaId,
}

#[cfg(test)]
mod tests {
    use source::{Span, Spanned};

    use crate::{
        Model, Word,
        field::{Domain, Field, access_modality::AccessModality},
        peripheral::Peripheral,
        register::Register,
        schema::Schema,
        variant::Variant,
    };

    const SPAN: Span = Span {
        source: 0,
        start: 0,
        end: 0,
    };

    #[test]
    fn structure() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(Peripheral {
            name: spanned("foo".to_string()),
            address: spanned(0x4000_0000),
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        });
        let bar = model.add_register(
            foo,
            Register {
                name: spanned("bar".to_string()),
                offset: spanned(0x4),
                reset: None,
                access: Vec::new(),
                docs: Vec::new(),
                leaky: false,
                span: SPAN,
            },
        );

        let baz = model.add_schema(Schema {
            name: Some(spanned("baz".to_string())),
            docs: Vec::new(),
            span: SPAN,
        });
        let a = model.add_agnostic_variant(baz, variant("A", 0));
        let b = model.add_read_variant(baz, variant("B", 1));
        let c = model.add_write_variant(baz, variant("C", 2));

        let qux = model.add_field(
            bar,
            Field {
                name: spanned("qux".to_string()),
                domain: spanned(Domain::new(1, 0).unwrap()),
                access_modality: spanned(AccessModality::ReadWrite),
                schema: Some(spanned(baz)),
                reset: None,
                docs: Vec::new(),
                leaky: false,
                span: SPAN,
            },
        );

        similar_asserts::assert_eq!(
            expected: (vec![foo], vec![bar], vec![qux]),
            found: (
                model.peripherals().collect::<Vec<_>>(),
                model.registers(foo).collect::<Vec<_>>(),
                model.fields(bar).collect::<Vec<_>>(),
            ),
            "expected the model to visit the peripheral, register, and field added to it",
        );
        similar_asserts::assert_eq!(
            expected: (vec![a], vec![b], vec![c]),
            found: (
                model.agnostic_variants(baz).collect::<Vec<_>>(),
                model.read_variants(baz).collect::<Vec<_>>(),
                model.write_variants(baz).collect::<Vec<_>>(),
            ),
            "expected the model to keep the access-agnostic, read, and write variants added to `baz` apart",
        );
        similar_asserts::assert_eq!(
            expected: (foo, bar, baz),
            found: (
                model.parent(bar),
                model.parent(qux),
                model.parent(b),
            ),
            "expected the model to produce the parents of `bar`, `qux`, and `b`",
        );
        similar_asserts::assert_eq!(
            expected: Some("baz"),
            found: model[*model[qux].schema.unwrap()]
                .name
                .as_ref()
                .map(|name| name.as_str()),
            "expected the schema of field `qux` to resolve to `baz`",
        );
    }

    #[test]
    fn domain() {
        similar_asserts::assert_eq!(
            expected: [Some(16), Some(1)],
            found: [Domain::new(15, 0), Domain::new(3, 3)].map(|domain| domain.map(Domain::width)),
            "expected `Domain::width` to count the bits of `15:0` and `3:3`",
        );
        similar_asserts::assert_eq!(
            expected: None,
            found: Domain::new(0, 15),
            "expected `Domain::new` to reject a least significant bit above the most significant bit",
        );
    }

    fn spanned<T>(inner: T) -> Spanned<T> {
        Spanned { inner, span: SPAN }
    }

    fn variant(name: &str, discriminant: Word) -> Variant {
        Variant {
            name: spanned(name.to_string()),
            discriminant: spanned(discriminant),
            inert: false,
            docs: Vec::new(),
            span: SPAN,
        }
    }
}
