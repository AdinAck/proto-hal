//! The MMIO device model.
//!
//! # Leaky
//!
//! An item is *leaky* when the model cannot encapsulate its hardware invariants or its description is knowingly
//! incomplete. Using a leaky item may introduce unsoundness, and any interface generated from the model reflects this
//! opportunity for unsoundness.

#![deny(missing_docs)]

pub mod access;
pub mod diagnostic;
pub mod field;
pub mod peripheral;
pub mod register;
pub mod schema;
pub mod variant;

mod validation;

use std::ops::{Index, IndexMut};

use derive_more::Display;

use crate::{
    field::{Field, FieldId, FieldNode},
    peripheral::{Peripheral, PeripheralId, PeripheralNode},
    register::{Register, RegisterId, RegisterNode},
    schema::{Schema, SchemaId, SchemaNode},
    variant::{Variant, VariantId, VariantNode},
};

/// The model of a device.
///
/// A model holds the device's [peripherals](peripheral), their [registers](register), and the registers'
/// [fields](field). Each field has an [access] modality, and may take its variants from a [schema].
///
/// Each node is added with an `add_*` method, which produces a handle to the node, and indexing the model by a handle
/// produces the node. A node refers to others by their handles, so it is added after the nodes it refers to, i.e. a
/// schema is added before the fields that assume it.
#[derive(Debug, Clone, Default)]
pub struct Model {
    peripherals: Vec<PeripheralNode>,
    registers: Vec<RegisterNode>,
    fields: Vec<FieldNode>,
    schemas: Vec<SchemaNode>,
    variants: Vec<VariantNode>,
}

impl Model {
    /// Add a peripheral, producing its handle.
    pub fn add_peripheral(&mut self, peripheral: Peripheral) -> PeripheralId {
        let id = PeripheralId(self.peripherals.len());

        self.peripherals.push(PeripheralNode {
            peripheral,
            registers: Vec::new(),
        });

        id
    }

    /// Add a register to `peripheral`, producing its handle.
    pub fn add_register(&mut self, peripheral: PeripheralId, register: Register) -> RegisterId {
        let id = RegisterId(self.registers.len());

        self.registers.push(RegisterNode {
            register,
            peripheral,
            fields: Vec::new(),
        });
        self.peripherals[peripheral.0].registers.push(id);

        id
    }

    /// Add a field to `register`, producing its handle.
    pub fn add_field(&mut self, register: RegisterId, field: Field) -> FieldId {
        let id = FieldId(self.fields.len());

        self.fields.push(FieldNode { field, register });
        self.registers[register.0].fields.push(id);

        id
    }

    /// Add a schema, producing its handle.
    pub fn add_schema(&mut self, schema: Schema) -> SchemaId {
        let id = SchemaId(self.schemas.len());

        self.schemas.push(SchemaNode {
            schema,
            variants: Vec::new(),
            read_variants: Vec::new(),
            write_variants: Vec::new(),
        });

        id
    }

    /// Add a variant to `schema`, of both directions, producing its handle.
    pub fn add_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        self.add_variant_among(schema, variant, |node| &mut node.variants)
    }

    /// Add a variant to `schema`, of reads only, producing its handle.
    pub fn add_read_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        self.add_variant_among(schema, variant, |node| &mut node.read_variants)
    }

    /// Add a variant to `schema`, of writes only, producing its handle.
    pub fn add_write_variant(&mut self, schema: SchemaId, variant: Variant) -> VariantId {
        self.add_variant_among(schema, variant, |node| &mut node.write_variants)
    }

    /// Add a variant to `schema`, placing it in the list `among` selects, and producing its handle.
    fn add_variant_among(
        &mut self,
        schema: SchemaId,
        variant: Variant,
        among: fn(&mut SchemaNode) -> &mut Vec<VariantId>,
    ) -> VariantId {
        let id = VariantId(self.variants.len());

        self.variants.push(VariantNode { variant, schema });
        among(&mut self.schemas[schema.0]).push(id);

        id
    }

    /// The peripherals, in the order added.
    pub fn peripherals(&self) -> impl Iterator<Item = PeripheralId> {
        (0..self.peripherals.len()).map(PeripheralId)
    }

    /// The registers of `peripheral`, in the order added.
    pub fn registers(&self, peripheral: PeripheralId) -> &[RegisterId] {
        &self.peripherals[peripheral.0].registers
    }

    /// The fields of `register`, in the order added.
    pub fn fields(&self, register: RegisterId) -> &[FieldId] {
        &self.registers[register.0].fields
    }

    /// The schemas, in the order added.
    pub fn schemas(&self) -> impl Iterator<Item = SchemaId> {
        (0..self.schemas.len()).map(SchemaId)
    }

    /// The variants of `schema` of both directions, in the order added.
    pub fn variants(&self, schema: SchemaId) -> &[VariantId] {
        &self.schemas[schema.0].variants
    }

    /// The variants of `schema` of reads only, in the order added.
    pub fn read_variants(&self, schema: SchemaId) -> &[VariantId] {
        &self.schemas[schema.0].read_variants
    }

    /// The variants of `schema` of writes only, in the order added.
    pub fn write_variants(&self, schema: SchemaId) -> &[VariantId] {
        &self.schemas[schema.0].write_variants
    }

    /// The peripheral `register` belongs to.
    pub fn peripheral_of(&self, register: RegisterId) -> PeripheralId {
        self.registers[register.0].peripheral
    }

    /// The register `field` belongs to.
    pub fn register_of(&self, field: FieldId) -> RegisterId {
        self.fields[field.0].register
    }

    /// The schema `variant` belongs to.
    pub fn schema_of(&self, variant: VariantId) -> SchemaId {
        self.variants[variant.0].schema
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

/// Implement model indexing for each node kind.
macro_rules! index {
    ($($id:ty => $nodes:ident.$node:ident: $ty:ty),* $(,)?) => {$(
        impl Index<$id> for Model {
            type Output = $ty;

            fn index(&self, id: $id) -> &$ty {
                &self.$nodes[id.0].$node
            }
        }

        impl IndexMut<$id> for Model {
            fn index_mut(&mut self, id: $id) -> &mut $ty {
                &mut self.$nodes[id.0].$node
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

#[cfg(test)]
mod tests {
    use source::{Span, Spanned};

    use crate::{
        Model,
        access::Access,
        field::{Domain, Field},
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
        let mut model = Model::default();

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
        let a = model.add_variant(baz, variant("A", 0));
        let b = model.add_read_variant(baz, variant("B", 1));
        let c = model.add_write_variant(baz, variant("C", 2));

        let qux = model.add_field(
            bar,
            Field {
                name: spanned("qux".to_string()),
                domain: spanned(Domain::new(1, 0).unwrap()),
                access: spanned(Access::ReadWrite),
                schema: Some(spanned(baz)),
                reset: None,
                docs: Vec::new(),
                leaky: false,
                span: SPAN,
            },
        );

        assert_eq!(model.peripherals().collect::<Vec<_>>(), [foo]);
        assert_eq!(model.registers(foo), [bar]);
        assert_eq!(model.fields(bar), [qux]);
        assert_eq!(
            (
                model.variants(baz),
                model.read_variants(baz),
                model.write_variants(baz)
            ),
            ([a].as_slice(), [b].as_slice(), [c].as_slice()),
            "variants should be kept by direction",
        );
        assert_eq!(
            (
                model.peripheral_of(bar),
                model.register_of(qux),
                model.schema_of(b)
            ),
            (foo, bar, baz),
            "nodes should know their parents",
        );
        assert_eq!(
            model[*model[qux].schema.unwrap()]
                .name
                .as_ref()
                .map(|name| name.as_str()),
            Some("baz"),
            "a field should refer to its schema",
        );
    }

    #[test]
    fn domain() {
        assert_eq!(Domain::new(15, 0).map(Domain::width), Some(16));
        assert_eq!(Domain::new(3, 3).map(Domain::width), Some(1));
        assert_eq!(
            Domain::new(0, 15),
            None,
            "a domain's most significant bit should come first"
        );
    }

    fn spanned<T>(inner: T) -> Spanned<T> {
        Spanned { inner, span: SPAN }
    }

    fn variant(name: &str, discriminant: u32) -> Variant {
        Variant {
            name: spanned(name.to_string()),
            discriminant: spanned(discriminant),
            inert: false,
            docs: Vec::new(),
            span: SPAN,
        }
    }
}
