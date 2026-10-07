//! The model components related to validating a [model](crate).

use std::collections::{HashMap, HashSet, hash_map::Entry};

use ::diagnostic::Diagnostic;
use source::Spanned;

use crate::{
    Model, Node,
    diagnostic::{duplicate, empty_schema, unused_schema},
    schema::SchemaId,
    variant::VariantId,
};

impl Model {
    /// Validate the model, producing every diagnostic.
    pub fn validate(&self) -> Vec<Diagnostic> {
        self.duplicate_names()
            .chain(self.empty_schemas())
            .chain(self.unused_schemas())
            .collect()
    }

    /// Find names given to more than one sibling.
    fn duplicate_names(&self) -> impl Iterator<Item = Diagnostic> {
        let peripherals = duplicates(
            Node::Peripheral,
            self.peripherals().map(|peripheral| &self[peripheral].name),
        );

        let registers = self.peripherals().flat_map(|peripheral| {
            duplicates(
                Node::Register,
                self.registers(peripheral)
                    .iter()
                    .map(|&register| &self[register].name),
            )
        });

        let fields = self
            .peripherals()
            .flat_map(|peripheral| self.registers(peripheral))
            .flat_map(|&register| {
                duplicates(
                    Node::Field,
                    self.fields(register).iter().map(|&field| &self[field].name),
                )
            });

        let variants = self.schemas().flat_map(|schema| {
            duplicates(
                Node::Variant,
                self.all_variants(schema).map(|variant| &self[variant].name),
            )
        });

        peripherals.chain(registers).chain(fields).chain(variants)
    }

    /// Find schemas without variants.
    fn empty_schemas(&self) -> impl Iterator<Item = Diagnostic> {
        self.schemas()
            .filter(|&schema| self.all_variants(schema).next().is_none())
            .map(|schema| empty_schema(self[schema].name.as_ref(), self[schema].span))
    }

    /// Find schemas no field uses.
    fn unused_schemas(&self) -> impl Iterator<Item = Diagnostic> {
        let used: HashSet<SchemaId> = self
            .fields
            .iter()
            .filter_map(|node| node.field.schema.map(|schema| *schema))
            .collect();

        self.schemas()
            .filter(move |schema| !used.contains(schema))
            .map(|schema| unused_schema(self[schema].name.as_ref(), self[schema].span))
    }

    /// The variants of `schema` of every direction.
    fn all_variants(&self, schema: SchemaId) -> impl Iterator<Item = VariantId> {
        self.variants(schema)
            .iter()
            .chain(self.read_variants(schema))
            .chain(self.write_variants(schema))
            .copied()
    }
}

/// Find the names given more than once among `names`, each of a sibling `node`.
fn duplicates<'a>(
    node: Node,
    names: impl IntoIterator<Item = &'a Spanned<String>>,
) -> impl Iterator<Item = Diagnostic> {
    let mut first = HashMap::new();

    names
        .into_iter()
        .filter_map(move |name| match first.entry(name.as_str()) {
            Entry::Occupied(entry) => Some(duplicate(node, name, *entry.get())),
            Entry::Vacant(entry) => {
                entry.insert(name.span);
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use source::{Span, Spanned};

    use crate::{
        Model,
        access::Access,
        diagnostic::Structure,
        field::{Domain, Field},
        peripheral::Peripheral,
        register::Register,
        schema::{Schema, SchemaId},
        variant::Variant,
    };

    const SPAN: Span = Span {
        source: 0,
        start: 0,
        end: 0,
    };

    #[test]
    fn duplicate_names() {
        let mut model = Model::default();

        let foo = model.add_peripheral(peripheral("foo"));
        model.add_peripheral(peripheral("foo"));
        let bar = model.add_register(foo, register("bar"));
        model.add_register(foo, register("bar"));
        model.add_field(bar, field("baz", None));
        model.add_field(bar, field("baz", None));

        let qux = model.add_schema(schema(Some("qux")));
        model.add_variant(qux, variant("A"));
        model.add_write_variant(qux, variant("A"));
        model.add_field(bar, field("quux", Some(qux)));

        let messages: Vec<_> = model
            .validate()
            .into_iter()
            .inspect(|diagnostic| {
                assert!(
                    diagnostic.is(Structure::Duplicate),
                    "only duplicates should be found"
                )
            })
            .map(|diagnostic| diagnostic.message)
            .collect();

        assert_eq!(
            messages,
            [
                "peripheral `foo` is defined more than once",
                "register `bar` is defined more than once",
                "field `baz` is defined more than once",
                "variant `A` is defined more than once",
            ],
            "every name given to more than one sibling should be found, across every direction of a schema",
        );
    }

    #[test]
    fn siblings() {
        let mut model = Model::default();

        let foo = model.add_peripheral(peripheral("foo"));
        let bar = model.add_peripheral(peripheral("bar"));
        model.add_register(foo, register("baz"));
        model.add_register(bar, register("baz"));

        assert!(
            model.validate().is_empty(),
            "the same name under different parents should not be a duplicate"
        );
    }

    #[test]
    fn schemas() {
        let mut model = Model::default();

        let foo = model.add_peripheral(peripheral("foo"));
        let bar = model.add_register(foo, register("bar"));
        let baz = model.add_schema(schema(Some("baz")));
        let qux = model.add_schema(schema(Some("qux")));
        model.add_variant(qux, variant("A"));
        model.add_field(bar, field("quux", Some(baz)));

        let diagnostics = model.validate();

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| &diagnostic.message)
                .collect::<Vec<_>>(),
            ["schema `baz` has no variants", "schema `qux` is never used"],
        );
        assert!(
            diagnostics[0].is(Structure::EmptySchema) && diagnostics[1].is(Structure::UnusedSchema),
            "an empty schema and an unused schema should be found",
        );
    }

    fn peripheral(name: &str) -> Peripheral {
        Peripheral {
            name: spanned(name.to_string()),
            address: spanned(0),
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        }
    }

    fn register(name: &str) -> Register {
        Register {
            name: spanned(name.to_string()),
            offset: spanned(0),
            reset: None,
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        }
    }

    fn field(name: &str, schema: Option<SchemaId>) -> Field {
        Field {
            name: spanned(name.to_string()),
            domain: spanned(Domain::new(0, 0).unwrap()),
            access: spanned(Access::Store),
            schema: schema.map(spanned),
            reset: None,
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        }
    }

    fn schema(name: Option<&str>) -> Schema {
        Schema {
            name: name.map(|name| spanned(name.to_string())),
            docs: Vec::new(),
            span: SPAN,
        }
    }

    fn variant(name: &str) -> Variant {
        Variant {
            name: spanned(name.to_string()),
            discriminant: spanned(0),
            inert: false,
            docs: Vec::new(),
            span: SPAN,
        }
    }

    fn spanned<T>(inner: T) -> Spanned<T> {
        Spanned { inner, span: SPAN }
    }
}
