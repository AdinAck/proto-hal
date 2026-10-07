//! The model components related to validating a [model](crate).

mod overlap;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    ops::Range,
};

use ::diagnostic::Diagnostic;
use source::Spanned;

use crate::{
    Model, Node, WideWord, Word, WordSize,
    diagnostic::{self, format_bits, format_bytes},
    peripheral::PeripheralId,
    register::RegisterId,
    schema::SchemaId,
    variant::VariantId,
};

impl Model {
    /// Validate the model, producing the diagnostics to be emitted.
    pub fn validate(&self) -> Vec<Diagnostic> {
        // structure-level diagnostics
        let structure = self
            .duplicate_names()
            .chain(self.empty_schemas())
            .chain(self.unused_schemas());

        match WordSize::new(*self.word_size) {
            Some(word) => structure.chain(self.placement(word)).collect(),
            None => structure
                .chain([diagnostic::invalid_word_size(&self.word_size)])
                .collect(),
        }
    }

    /// Validate the placement of [memory-mapped items](crate#items) according to the provided valid
    /// [word size](crate#word-size).
    fn placement(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        self.unaligned_addresses(word_size)
            .chain(self.out_of_range_addresses(word_size))
            .chain(self.overlaps(word_size))
            .chain(self.exceeded_domains(word_size))
            .chain(self.invalid_access_widths(word_size))
    }

    /// Validate that sibling names are *unique*.
    fn duplicate_names(&self) -> impl Iterator<Item = Diagnostic> {
        let peripherals = duplicates(
            Node::Peripheral,
            self.peripherals().map(|peripheral| &self[peripheral].name),
        );

        let registers = self.peripherals().flat_map(|peripheral| {
            duplicates(
                Node::Register,
                self.registers(peripheral)
                    .map(|register| &self[register].name),
            )
        });

        let fields = self.all_registers().flat_map(|register| {
            duplicates(
                Node::Field,
                self.fields(register).map(|field| &self[field].name),
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

    /// Validate that schemas are *non-empty* (define at least one variant).
    fn empty_schemas(&self) -> impl Iterator<Item = Diagnostic> {
        self.schemas()
            .filter(|&schema| self.all_variants(schema).next().is_none())
            .map(|schema| diagnostic::empty_schema(&self[schema]))
    }

    /// Validate that schemas are *used* by at least one field.
    fn unused_schemas(&self) -> impl Iterator<Item = Diagnostic> {
        let used = self
            .fields
            .iter()
            .filter_map(|node| node.field.schema.map(|schema| *schema))
            .collect::<HashSet<_>>();

        self.schemas()
            .filter(move |schema| !used.contains(schema))
            .map(|schema| diagnostic::unused_schema(&self[schema]))
    }

    /// Validate that peripheral addresses and register offsets are [word](crate#word-size) aligned.
    fn unaligned_addresses(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        let peripherals = self
            .peripherals()
            .map(|peripheral| (Node::Peripheral, &self[peripheral].address));

        let registers = self
            .all_registers()
            .map(|register| (Node::Register, &self[register].offset));

        peripherals
            .chain(registers)
            .filter(move |(.., address)| !address.is_multiple_of(word_size.bytes().into()))
            .map(move |(node, address)| diagnostic::address_unaligned(node, address, word_size))
    }

    /// Validate that peripheral and register addresses reside within the permitted
    /// [address space](crate#address-space).
    ///
    /// *Note: Diagnostics will not be emitted for registers within offending peripherals.*
    fn out_of_range_addresses(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        let last = word_size.last_address();

        let peripherals = self.peripherals().map(|peripheral| {
            let address = &self[peripheral].address;

            let address = Spanned {
                inner: WideWord::from(**address),
                span: address.span,
            };

            (Node::Peripheral, address)
        });

        let registers = self
            .peripherals()
            .filter(move |&peripheral| *self[peripheral].address <= last)
            .flat_map(move |peripheral| {
                self.registers(peripheral).map(move |register| {
                    let offset = &self[register].offset;

                    let address = Spanned {
                        inner: self.register_address(register),
                        span: offset.span,
                    };

                    (Node::Register, address)
                })
            });

        peripherals
            .chain(registers)
            .filter(move |(.., address)| **address > WideWord::from(last))
            .map(move |(node, address)| diagnostic::address_out_of_range(node, address, word_size))
    }

    /// Validate that siblings are *disjoint* (occupy no common region).
    fn overlaps(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        let peripherals = self.peripherals().map(move |peripheral| overlap::Sibling {
            handle: peripheral,
            range: self.domain(peripheral, word_size),
        });

        let peripheral_overlaps = overlap::shared_regions(peripherals).map(move |region| {
            let range = &region.range;

            // the registers occupying the region, of each peripheral occupying it
            let occupants = region.occupants.iter().map(|&peripheral| {
                let registers = self.registers(peripheral).filter(move |&register| {
                    // invariant: a register address is at most `2 * Word::MAX`, so adding the word size fits in a
                    //            wide word
                    let start = self.register_address(register);

                    start < range.end && range.start < start + WideWord::from(word_size.bytes())
                });

                (&self[peripheral], registers.map(|register| &self[register]))
            });

            diagnostic::peripheral_overlap(occupants, format_bytes(range, word_size))
        });

        let register_overlaps = self.peripherals().flat_map(move |peripheral| {
            let registers = self.registers(peripheral).map(move |register| {
                // invariant: an offset is at most `Word::MAX`, so adding the word size fits in a wide word
                let start = WideWord::from(*self[register].offset);

                overlap::Sibling {
                    handle: register,
                    range: start..start + WideWord::from(word_size.bytes()),
                }
            });

            overlap::shared_regions(registers).map(move |region| {
                diagnostic::register_overlap(
                    region.occupants.iter().map(|&register| &self[register]),
                    format_bytes(&region.range, word_size),
                )
            })
        });

        // TODO: fields that are superpositioned, as their ontological entitlement spaces contradict, are exempt
        let field_overlaps = self.all_registers().flat_map(move |register| {
            let fields = self.fields(register).map(|field| {
                let domain = *self[field].domain;

                // invariant: bit indices are at most `u8::MAX`, so the exclusive end fits in a wide word
                overlap::Sibling {
                    handle: field,
                    range: WideWord::from(domain.lsb())..WideWord::from(domain.msb()) + 1,
                }
            });

            overlap::shared_regions(fields).map(move |region| {
                diagnostic::field_overlap(
                    region.occupants.iter().map(|&field| &self[field]),
                    format_bits(&region.range),
                )
            })
        });

        peripheral_overlaps
            .chain(register_overlaps)
            .chain(field_overlaps)
    }

    /// Validate that field domains reside within their parent register's [domain](crate#domain).
    fn exceeded_domains(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        self.all_registers()
            .flat_map(|register| self.fields(register))
            .map(|field| &self[field])
            .filter(move |field| !word_size.domain().contains(*field.domain))
            .map(move |field| diagnostic::exceeds_domain(field, word_size.domain()))
    }

    /// Validate that register access widths divide the [word size](crate#word-size).
    fn invalid_access_widths(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        self.all_registers()
            .flat_map(|register| &self[register].access)
            .filter(move |width| !Word::from(word_size.bytes()).is_multiple_of(***width))
            .map(move |width| diagnostic::invalid_access_width(width, word_size))
    }

    /// The address of the provided register.
    fn register_address(&self, register: RegisterId) -> WideWord {
        // invariant: an address and an offset are each at most `Word::MAX`, so their sum fits in a wide word
        WideWord::from(*self[self.parent(register)].address)
            + WideWord::from(*self[register].offset)
    }

    /// The [domain](crate#domain) of the provided peripheral.
    fn domain(&self, peripheral: PeripheralId, word_size: WordSize) -> Range<WideWord> {
        // invariant: the base address, an offset, and the word size are each at most `Word::MAX`, so their sum fits
        //            in a wide word
        let address = WideWord::from(*self[peripheral].address);

        let end = self
            .registers(peripheral)
            .map(|register| {
                WideWord::from(*self[register].offset) + WideWord::from(word_size.bytes())
            })
            .max()
            .unwrap_or(0);

        address..address + end
    }

    /// An iterator visiting all registers, peripheral by peripheral, in the order they were added.
    fn all_registers(&self) -> impl Iterator<Item = RegisterId> {
        self.peripherals()
            .flat_map(|peripheral| self.registers(peripheral))
    }

    /// An iterator visiting all variants defined by the provided schema.
    fn all_variants(&self, schema: SchemaId) -> impl Iterator<Item = VariantId> {
        self.agnostic_variants(schema)
            .chain(self.read_variants(schema))
            .chain(self.write_variants(schema))
    }
}

/// Find the names given more than once among the provided names of siblings of the provided node kind.
fn duplicates<'a>(
    node: Node,
    names: impl IntoIterator<Item = &'a Spanned<String>>,
) -> impl Iterator<Item = Diagnostic> {
    let mut first = HashMap::new();

    names
        .into_iter()
        .filter_map(move |name| match first.entry(name.as_str()) {
            Entry::Occupied(entry) => Some(diagnostic::duplicate(node, name, *entry.get())),
            Entry::Vacant(entry) => {
                entry.insert(name.span);
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use ::diagnostic::{Code, Diagnostic, Kind as _};
    use source::{Span, Spanned};

    use crate::{
        Model, Word,
        diagnostic::Structure,
        field::{Domain, Field, access_modality::AccessModality},
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
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        model.add_peripheral(peripheral("foo", 0x1000));
        let bar = model.add_register(foo, register("bar", 0x0));
        model.add_register(foo, register("bar", 0x4));
        model.add_field(bar, field("baz", (0, 0), None));
        model.add_field(bar, field("baz", (1, 1), None));

        let qux = model.add_schema(schema(Some("qux")));
        model.add_agnostic_variant(qux, variant("A"));
        model.add_write_variant(qux, variant("A"));
        model.add_field(bar, field("quux", (2, 2), Some(qux)));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::Duplicate.code(), "peripheral `foo` is defined more than once"),
                (Structure::Duplicate.code(), "register `bar` is defined more than once"),
                (Structure::Duplicate.code(), "field `baz` is defined more than once"),
                (Structure::Duplicate.code(), "variant `A` is defined more than once"),
            ],
            found: summary(&diagnostics),
            "expected a diagnostic to be emitted for each duplicate definition of `foo`, `bar`, `baz`, and `A`",
        );
    }

    #[test]
    fn siblings() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_peripheral(peripheral("bar", 0x1000));
        model.add_register(foo, register("baz", 0x0));
        model.add_register(bar, register("baz", 0x0));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: Vec::<(Code, &str)>::new(),
            found: summary(&diagnostics),
            "expected no diagnostics to be emitted for registers in different peripherals with a shared name",
        );
    }

    #[test]
    fn schemas() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        let qux = model.add_schema(schema(Some("qux")));
        model.add_agnostic_variant(qux, variant("A"));
        model.add_field(bar, field("quux", (0, 0), Some(baz)));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::EmptySchema.code(), "schema `baz` has no variants"),
                (Structure::UnusedSchema.code(), "schema `qux` is never used"),
            ],
            found: summary(&diagnostics),
            "expected an empty schema diagnostic for `baz` and an unused schema diagnostic for `qux`",
        );
    }

    #[test]
    fn placement() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_peripheral(peripheral("bar", 0x4));
        let grault = model.add_peripheral(peripheral("grault", 0x1002));
        let baz = model.add_register(foo, register("baz", 0x0));
        model.add_register(foo, register("qux", 0x4));
        model.add_register(foo, register("quux", 0x4));
        model.add_register(bar, register("corge", 0x0));
        model.add_register(grault, register("garply", 0x2));
        model.add_field(baz, field("a", (3, 0), None));
        model.add_field(baz, field("b", (5, 2), None));
        model.add_field(baz, field("c", (35, 32), None));
        model.add_field(baz, field("d", (4, 3), None));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::AddressUnaligned.code(), "peripheral address must be word aligned"),
                (Structure::AddressUnaligned.code(), "register address must be word aligned"),
                (
                    Structure::PeripheralOverlap.code(),
                    "peripherals `foo` and `bar` overlap, occupying `0x0000_0004..0x0000_0008`",
                ),
                (
                    Structure::RegisterOverlap.code(),
                    "registers `qux` and `quux` overlap, occupying `0x0000_0004..0x0000_0008`",
                ),
                (Structure::FieldOverlap.code(), "fields `a` and `b` overlap, occupying `2:2`"),
                (Structure::FieldOverlap.code(), "fields `a`, `b`, and `d` overlap, occupying `3:3`"),
                (Structure::FieldOverlap.code(), "fields `b` and `d` overlap, occupying `4:4`"),
                (
                    Structure::ExceedsDomain.code(),
                    "field `c` with domain `35:32` exceeds parent register with domain `31:0`",
                ),
            ],
            found: summary(&diagnostics),
            "expected a diagnostic for each unaligned address, overlap region, and exceeding field domain",
        );
        similar_asserts::assert_eq!(
            expected: vec![
                "`bar` placed here",
                "register `foo::qux` overlaps",
                "register `foo::quux` overlaps",
                "register `bar::corge` overlaps",
            ],
            found: diagnostics[2]
                .supporting_labels
                .iter()
                .map(|label| label.message.as_str())
                .collect::<Vec<_>>(),
            "expected the overlap diagnostic of `foo` and `bar` to label the registers occupying the overlap region",
        );
    }

    #[test]
    fn word_size() {
        let mut model = Model::new(spanned(2));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x2));
        model.add_register(foo, register("baz", 0x3));
        model.add_field(bar, field("qux", (16, 0), None));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::AddressUnaligned.code(), "register address must be word aligned"),
                (
                    Structure::RegisterOverlap.code(),
                    "registers `bar` and `baz` overlap, occupying `0x0003..0x0004`",
                ),
                (
                    Structure::ExceedsDomain.code(),
                    "field `qux` with domain `16:0` exceeds parent register with domain `15:0`",
                ),
            ],
            found: summary(&diagnostics),
            "expected registers and fields to be placed according to word size `2`",
        );
    }

    #[test]
    fn invalid_word_sizes() {
        for word_size in [0, 9] {
            let mut model = Model::new(spanned(word_size));

            let foo = model.add_peripheral(peripheral("foo", 0x1));
            model.add_register(foo, register("bar", 0x0));
            model.add_register(foo, register("bar", 0x1));

            let diagnostics = model.validate();

            let note = format!("word size `{word_size}` does not satisfy `1 <= word size <= 8`");

            similar_asserts::assert_eq!(
                expected: (
                    vec![
                        (Structure::Duplicate.code(), "register `bar` is defined more than once"),
                        (Structure::InvalidWordSize.code(), "word size must be from 1 to 8 bytes"),
                    ],
                    vec![note.as_str()],
                ),
                found: (
                    summary(&diagnostics),
                    diagnostics
                        .iter()
                        .flat_map(|diagnostic| &diagnostic.notes)
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                ),
                "expected word size `{word_size}` to be rejected with a note, and placement to be skipped",
            );
        }
    }

    #[test]
    fn address_space() {
        for (word_size, peripherals, expected) in [
            (
                2,
                [("foo", 0x2_0000, 0x0), ("bar", 0xfff0, 0x10)].as_slice(),
                [
                    "address `0x0002_0000` does not satisfy `address <= 0xffff`",
                    "address `0x0001_0000` does not satisfy `address <= 0xffff`",
                ]
                .as_slice(),
            ),
            (
                8,
                [("foo", 0xffff_ffff_ffff_fff8, 0x8)].as_slice(),
                ["address `0x0001_0000_0000_0000_0000` does not satisfy `address <= 0xffff_ffff_ffff_ffff`"]
                    .as_slice(),
            ),
        ] {
            let mut model = Model::new(spanned(word_size));

            for &(name, address, offset) in peripherals {
                let peripheral = model.add_peripheral(peripheral(name, address));
                model.add_register(peripheral, register("baz", offset));
            }

            let diagnostics = model.validate();

            similar_asserts::assert_eq!(
                expected: expected
                    .iter()
                    .map(|&note| (Structure::AddressOutOfRange.code(), note))
                    .collect::<Vec<_>>(),
                found: diagnostics
                    .iter()
                    .flat_map(|diagnostic| {
                        diagnostic
                            .notes
                            .iter()
                            .map(|note| (diagnostic.id.code, note.as_str()))
                    })
                    .collect::<Vec<_>>(),
                "expected an out-of-range diagnostic for each peripheral or register outside the address space of word \
                 size `{word_size}`, skipping registers of out-of-range peripherals",
            );
        }
    }

    #[test]
    fn access_widths() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        model.add_register(
            foo,
            Register {
                access: [1, 2, 3, 4, 0].map(spanned).to_vec(),
                ..register("bar", 0x0)
            },
        );

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::InvalidAccessWidth.code(), "access width `3` does not divide the word size `4`"),
                (Structure::InvalidAccessWidth.code(), "access width `0` does not divide the word size `4`"),
            ],
            found: summary(&diagnostics),
            "expected an invalid access width diagnostic for each access width of `bar` that does not divide the word \
             size",
        );
    }

    /// The code and message of each of the provided diagnostics.
    fn summary<'a>(diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> Vec<(Code, &'a str)> {
        diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.id.code, diagnostic.message.as_str()))
            .collect()
    }

    fn spanned<T>(inner: T) -> Spanned<T> {
        Spanned { inner, span: SPAN }
    }

    fn peripheral(name: &str, address: Word) -> Peripheral {
        Peripheral {
            name: spanned(name.to_string()),
            address: spanned(address),
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        }
    }

    fn register(name: &str, offset: Word) -> Register {
        Register {
            name: spanned(name.to_string()),
            offset: spanned(offset),
            reset: None,
            access: Vec::new(),
            docs: Vec::new(),
            leaky: false,
            span: SPAN,
        }
    }

    fn field(name: &str, (msb, lsb): (u8, u8), schema: Option<SchemaId>) -> Field {
        Field {
            name: spanned(name.to_string()),
            domain: spanned(Domain::new(msb, lsb).unwrap()),
            access_modality: spanned(AccessModality::Store),
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
}
