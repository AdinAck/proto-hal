//! The model components related to validating a [model](crate).

mod overlap;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    ops::Range,
};

use ::diagnostic::Diagnostic;
use derive_more::Deref;
use source::Spanned;

use crate::{
    Model, Node, WideWord, Word,
    diagnostic::{self, Inexposable, format_bits, format_bytes},
    entitlement::{Entitlement, Space},
    field::{Field, access_modality},
    peripheral::PeripheralId,
    register::RegisterId,
    schema::{AccessSpace, SchemaId},
    variant::{AccessQualifier, VariantId},
    word_size::WordSize,
};

impl Model {
    /// Validate the model, producing the diagnostics to be emitted.
    pub fn validate(&self) -> Vec<Diagnostic> {
        Validation::new(self).diagnostics()
    }
}

/// The context of validating a [model](Model), containing data derived from the model once per validation.
#[derive(Deref)]
struct Validation<'a> {
    #[deref]
    model: &'a Model,
    /// A map from each schema and discriminant to the [access-agnostic](crate::variant#access-specificity) variant
    /// the discriminant identifies within the schema.
    identified_variants: HashMap<(SchemaId, Word), VariantId>,
}

impl<'a> Validation<'a> {
    /// Create the context for performing validation of the provided model.
    fn new(model: &'a Model) -> Self {
        // note: a schema may define overlapping access-agnostic variants, which validation reports, in which case the
        //       discriminant identifies the variant *last* added to the map
        let identified_variants = model
            .schemas()
            .flat_map(|schema| {
                model
                    .agnostic_variants(schema)
                    .map(move |variant| ((schema, *model[variant].discriminant), variant))
            })
            .collect();

        Self {
            model,
            identified_variants,
        }
    }

    /// Validate the model, producing the diagnostics to be emitted.
    fn diagnostics(&self) -> Vec<Diagnostic> {
        // structure-level diagnostics
        let structure = self
            .duplicate_names()
            .chain(self.empty_schemas())
            .chain(self.unused_schemas())
            .chain(self.variant_overlaps())
            .chain(self.inert_read_variants())
            .chain(self.schema())
            .chain(self.resets())
            .chain(self.entitlements());

        match WordSize::new(*self.word_size) {
            Some(word) => structure.chain(self.placement(word)).collect(),
            None => structure
                .chain([diagnostic::invalid_word_size(&self.word_size)])
                .collect(),
        }
    }

    /// Validate the [schemas](crate::schema) exposed by [fields](crate::field).
    fn schema(&self) -> impl Iterator<Item = Diagnostic> {
        self.out_of_range_discriminants()
            .chain(self.incompatible_fields())
            .chain(self.missing_readable_variants())
            .chain(self.missing_writable_variants())
    }

    /// Validate the [resets](crate::field#resets) defined by [fields](crate::field).
    fn resets(&self) -> impl Iterator<Item = Diagnostic> {
        self.out_of_range_resets()
            .chain(self.ignored_resets())
            .chain(self.uninhabitable_resets())
            .chain(self.missing_resets())
    }

    /// Validate the [entitlements](crate::entitlement) imposed by peripherals, fields, and variants.
    fn entitlements(&self) -> impl Iterator<Item = Diagnostic> {
        self.uninhabitable_entitlements()
            .chain(self.out_of_range_entitlements())
            .chain(self.unresolvable_entitlements())
            .chain(self.unpermitted_entitlement_spaces())
            .chain(self.foreign_statewise_variants())
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

        let fields = self.iter_registers().flat_map(|register| {
            duplicates(
                Node::Field,
                self.fields(register).map(|field| &self[field].name),
            )
        });

        let variants = self.schemas().flat_map(|schema| {
            duplicates(
                Node::Variant,
                self.variants(schema).map(|variant| &self[variant].name),
            )
        });

        peripherals.chain(registers).chain(fields).chain(variants)
    }

    /// Validate that schemas are *non-empty* (define at least one variant).
    fn empty_schemas(&self) -> impl Iterator<Item = Diagnostic> {
        self.schemas()
            .filter(|&schema| self.variants(schema).next().is_none())
            .map(|schema| diagnostic::empty_schema(&self[schema], self.defining_field(schema)))
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

    /// Validate that schemas are *consistent* (define no overlapping variants).
    fn variant_overlaps(&self) -> impl Iterator<Item = Diagnostic> {
        self.schemas().flat_map(move |schema| {
            let mut write = self
                .discriminant_overlaps(self.writable_variants(schema))
                .collect::<Vec<_>>();

            // note: the same variants overlapping in both access spaces are access-agnostic, so one diagnostic without
            //       an access space describes both overlaps
            let read = self
                .discriminant_overlaps(self.readable_variants(schema))
                .map(
                    |region| match write.iter().position(|other| *other == region) {
                        Some(index) => (None, write.remove(index)),
                        None => (Some(AccessSpace::Read), region),
                    },
                )
                .collect::<Vec<_>>();

            read.into_iter()
                .chain(
                    write
                        .into_iter()
                        .map(|region| (Some(AccessSpace::Write), region)),
                )
                .map(move |(space, region)| {
                    diagnostic::variant_overlap(
                        region.occupants.iter().map(|&variant| &self[variant]),
                        space,
                    )
                })
        })
    }

    /// Validate that read variants are not [inert](crate::variant#inert).
    fn inert_read_variants(&self) -> impl Iterator<Item = Diagnostic> {
        self.schemas()
            .flat_map(|schema| self.read_variants(schema))
            .map(|variant| &self[variant])
            .filter(|variant| variant.inert.is_some())
            .map(diagnostic::inert_read_variant)
    }

    /// Validate that the variants exposed by a field are assigned discriminants the field [domain](crate#domain) can
    /// represent.
    fn out_of_range_discriminants(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_enumerated_fields()
            .flat_map(move |(field, schema)| {
                self.variants(schema)
                    .map(|variant| &self[variant])
                    .filter(|variant| *variant.discriminant > field.domain.max_value())
                    .map(|variant| diagnostic::discriminant_out_of_range(variant, field))
            })
    }

    /// Validate that fields are [*compatible*](crate::field#compatibility) with the schemas defining their variants.
    fn incompatible_fields(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_enumerated_fields()
            .flat_map(move |(field, schema)| {
                let inexposable = self
                    .variants(schema)
                    .filter_map(|variant| {
                        let qualifier = self.offending_qualifier(variant, field)?;

                        Some(diagnostic::inexposable_variant(
                            &self[variant],
                            field,
                            qualifier,
                        ))
                    })
                    .collect::<Vec<_>>();

                // note: a field mustn't cause errors within an assumed schema, so the variants the field cannot expose
                //       become children of one error on the field
                match self[schema].name {
                    Some(..) if !inexposable.is_empty() => {
                        vec![diagnostic::incompatible_field(
                            field,
                            &self[schema],
                            inexposable,
                        )]
                    }
                    _ => inexposable,
                }
            })
    }

    /// Validate that fields [supporting](crate::field#compatibility) read access expose
    /// [readable](crate::schema#access-specificity) variants.
    fn missing_readable_variants(&self) -> impl Iterator<Item = Diagnostic> {
        use access_modality::*;

        self.iter_enumerated_fields()
            .filter(|(field, ..)| *field.access_modality != Write)
            .filter(move |&(.., schema)| self.readable_variants(schema).next().is_none())
            .map(move |(field, schema)| diagnostic::no_readable_variants(field, &self[schema]))
    }

    /// Validate that fields [supporting](crate::field#compatibility) write access expose
    /// [writable](crate::schema#access-specificity) variants.
    fn missing_writable_variants(&self) -> impl Iterator<Item = Diagnostic> {
        use access_modality::*;

        self.iter_enumerated_fields()
            .filter(|(field, ..)| *field.access_modality != Read)
            .filter(move |&(.., schema)| self.writable_variants(schema).next().is_none())
            .map(move |(field, schema)| diagnostic::no_writable_variants(field, &self[schema]))
    }

    /// Validate that the resets of [numeric](crate::field#numericity) [store fields](crate::field#resets) are
    /// representable by the [domain](crate#domain) of the defining field.
    fn out_of_range_resets(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_reset_values()
            .filter(|(field, value)| field.schema.is_none() && **value > field.domain.max_value())
            .map(|(field, value)| diagnostic::reset_out_of_range(value, field))
    }

    /// Validate that resets are defined by [store fields](crate::field#resets) alone.
    fn ignored_resets(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_fields()
            .filter(|field| field.reset.is_some() && !field.access_modality.is_store())
            .map(diagnostic::ignored_reset)
    }

    /// Validate that the resets of [enumerated](crate::field#numericity) [store fields](crate::field#resets) identify a
    /// variant exposed by the defining field.
    fn uninhabitable_resets(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_reset_values()
            .filter(move |(field, value)| {
                field.schema.is_some_and(|schema| {
                    !self.identified_variants.contains_key(&(*schema, **value))
                })
            })
            .map(|(field, value)| diagnostic::uninhabitable_reset(value, field))
    }

    // TODO: fields with modality `volatile store` whose hardware write entitlement space is tautological or
    //       satisfied on reset cannot define a reset
    /// Validate that [store fields](crate::field#resets) define a reset.
    fn missing_resets(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_fields()
            .filter(|field| field.access_modality.is_store() && field.reset.is_none())
            .map(diagnostic::missing_reset)
    }

    /// Validate that the entitlement values of [enumerated](crate::field#numericity) fields identify a variant exposed
    /// by the implicated field.
    fn uninhabitable_entitlements(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_entitlements().filter_map(move |entitlement| {
            let field = &self[entitlement.field];
            let schema = *field.schema?;

            (!self
                .identified_variants
                .contains_key(&(schema, entitlement.state)))
            .then(|| {
                diagnostic::uninhabitable_entitlement(
                    Spanned {
                        inner: entitlement.state,
                        span: entitlement.span,
                    },
                    field,
                )
            })
        })
    }

    /// Validate that the entitlement values of [numeric](crate::field#numericity) fields are representable by the
    /// [domain](crate#domain) of the implicated field.
    fn out_of_range_entitlements(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_entitlements().filter_map(move |entitlement| {
            let value = entitlement.state;
            let field = &self[entitlement.field];

            (field.schema.is_none() && value > field.domain.max_value()).then(|| {
                diagnostic::entitlement_out_of_range(
                    Spanned {
                        inner: value,
                        span: entitlement.span,
                    },
                    field,
                )
            })
        })
    }

    /// Validate that entitlements implicate [resolvable](crate::field#resolvability) fields, as only resolvable fields
    /// may inhabit states.
    fn unresolvable_entitlements(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_entitlements()
            .map(move |entitlement| (entitlement, &self[entitlement.field]))
            .filter(|(.., field)| !is_resolvable(field))
            .map(|(entitlement, field)| diagnostic::unresolvable_entitlement(entitlement, field))
    }

    /// Validate that fields impose [affordance](crate::entitlement#affordance) entitlements only when the field
    /// [supports](crate::field#compatibility) write access, and [hardware
    /// affordance](crate::entitlement#hardware-affordance) entitlements only with modality `volatile store`.
    fn unpermitted_entitlement_spaces(&self) -> impl Iterator<Item = Diagnostic> {
        use access_modality::*;

        let affordance = self
            .iter_fields()
            .filter(|field| {
                field.affordance_entitlements.is_some() && *field.access_modality == Read
            })
            .map(diagnostic::unpermitted_affordance);

        let hardware_affordance = self
            .iter_fields()
            .filter(|field| {
                field.hardware_affordance_entitlements.is_some()
                    && *field.access_modality != VolatileStore
            })
            .map(diagnostic::unpermitted_hardware_affordance);

        affordance.chain(hardware_affordance)
    }

    /// Validate that the variants imposing [statewise](crate::entitlement#statewise) entitlements within a field are
    /// exposed by the field, and so not *foreign* to the field.
    fn foreign_statewise_variants(&self) -> impl Iterator<Item = Diagnostic> {
        self.iter_fields().flat_map(move |field| {
            field
                .statewise_entitlements
                .iter()
                .filter(move |&(&variant, ..)| self.is_foreign(variant, field))
                .map(move |(&variant, space)| {
                    diagnostic::foreign_statewise_variant(&self[variant], space, field)
                })
        })
    }

    /// Validate that peripheral addresses and register offsets are [word](crate#word-size) aligned.
    fn unaligned_addresses(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        let peripherals = self
            .peripherals()
            .map(|peripheral| (Node::Peripheral, &self[peripheral].address));

        let registers = self
            .iter_registers()
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
        let field_overlaps = self.iter_registers().flat_map(move |register| {
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
        self.iter_registers()
            .flat_map(|register| self.fields(register))
            .map(|field| &self[field])
            .filter(move |field| !word_size.domain().contains(*field.domain))
            .map(move |field| diagnostic::exceeds_domain(field, word_size.domain()))
    }

    /// Validate that register access widths divide the [word size](crate#word-size).
    fn invalid_access_widths(&self, word_size: WordSize) -> impl Iterator<Item = Diagnostic> {
        self.iter_registers()
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

    /// The qualifier preventing the provided field from exposing the provided variant, if any.
    fn offending_qualifier(&self, variant: VariantId, field: &Field) -> Option<Inexposable<'_>> {
        use access_modality::*;

        let variant = &self[variant];

        match (*field.access_modality, variant.access.map(|access| *access)) {
            (Read, Some(AccessQualifier::Write))
            | (Write, Some(AccessQualifier::Read))
            | (Store | VolatileStore, Some(..)) => variant.access.as_ref().map(Inexposable::Access),
            (Read | Store | VolatileStore, None) => variant.inert.as_ref().map(Inexposable::Inert),
            (..) => None,
        }
    }

    /// Determine whether the provided variant is [foreign](crate::field#schema) to the provided field or not.
    fn is_foreign(&self, variant: VariantId, field: &Field) -> bool {
        field
            .schema
            .is_none_or(|schema| *schema != self.parent(variant))
    }

    /// Find the regions of the discriminant space occupied by more than one of the provided variants.
    fn discriminant_overlaps(
        &self,
        variants: impl IntoIterator<Item = VariantId>,
    ) -> impl Iterator<Item = overlap::Region<VariantId>> {
        let variants = variants.into_iter().map(|variant| {
            // invariant: a discriminant is at most `Word::MAX`, so the exclusive end fits in a wide word
            let discriminant = WideWord::from(*self[variant].discriminant);

            overlap::Sibling {
                handle: variant,
                range: discriminant..discriminant + 1,
            }
        });

        overlap::shared_regions(variants)
    }

    // model sweeps
    // note: not to be confused with *queries* which are located in the crate root

    /// An iterator visiting all registers, peripheral by peripheral, in the order they were added.
    fn iter_registers(&self) -> impl Iterator<Item = RegisterId> {
        self.peripherals()
            .flat_map(|peripheral| self.registers(peripheral))
    }

    /// Retreieve the field defining the provided schema. If the schema is not [inherent](crate::field#schema) or no
    /// such defining field exists, returns `None`.
    fn defining_field(&self, schema: SchemaId) -> Option<&Field> {
        if self[schema].name.is_some() {
            return None;
        }

        // invariant: an inherent schema is exposed only by the field defining it
        self.fields
            .iter()
            .map(|node| &node.field)
            .find(|field| field.schema.is_some_and(|exposed| *exposed == schema))
    }

    /// An iterator visiting all [enumerated](crate::field#numericity) fields, register by register, in the order they
    /// were added, paired with the schema each field exposes.
    fn iter_enumerated_fields(&self) -> impl Iterator<Item = (&Field, SchemaId)> {
        self.iter_fields()
            .filter_map(|field| Some((field, *field.schema?)))
    }

    /// An iterator visiting all fields, register by register, in the order they were added.
    fn iter_fields(&self) -> impl Iterator<Item = &Field> {
        self.iter_registers()
            .flat_map(|register| self.fields(register))
            .map(|field| &self[field])
    }

    /// An iterator visiting all [store fields](crate::field#resets) defining a reset, register by register, in the
    /// order they were added, paired with the respective reset.
    fn iter_reset_values(&self) -> impl Iterator<Item = (&Field, Spanned<Word>)> {
        self.iter_fields()
            .filter(|field| field.access_modality.is_store())
            .filter_map(|field| Some((field, field.reset?)))
    }

    /// An iterator visiting all entitlement spaces imposed by peripherals, then those imposed by fields and by variants
    /// within fields, grouped by item in the order the items were added.
    fn iter_entitlement_spaces(&self) -> impl Iterator<Item = &Spanned<Space>> {
        let peripherals = self
            .peripherals()
            .filter_map(|peripheral| self[peripheral].ontological_entitlements.as_ref());

        let fields = self.iter_fields().flat_map(|field| {
            field
                .ontological_entitlements
                .iter()
                .chain(&field.affordance_entitlements)
                .chain(&field.hardware_affordance_entitlements)
                .chain(field.statewise_entitlements.values())
        });

        peripherals.chain(fields)
    }

    /// An iterator visiting all entitlements listed by the patterns of the
    /// [entitlement spaces](Self::iter_entitlement_spaces), in the same order.
    fn iter_entitlements(&self) -> impl Iterator<Item = &Spanned<Entitlement>> {
        self.iter_entitlement_spaces()
            .flat_map(|space| &space.patterns)
            .flat_map(|pattern| &pattern.entitlements)
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

// TODO: a hardware affordance entitlement space that is a tautology also leaves a field with modality `volatile store`
//       unresolvable, which the entitlement algebra will determine
/// Determine whether the provided field is [resolvable](crate::field#resolvability) or not.
fn is_resolvable(field: &Field) -> bool {
    use access_modality::*;

    match *field.access_modality {
        Store => true,
        VolatileStore => field.hardware_affordance_entitlements.is_some(),
        Read | Write | ReadWrite => false,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, iter};

    use ::diagnostic::{Code, Diagnostic, Kind as _, Severity};
    use source::{Span, Spanned};

    use crate::{
        Model, Word,
        diagnostic::Structure,
        entitlement::{Entitlement, Pattern, Space},
        field::{Domain, Field, access_modality},
        peripheral::Peripheral,
        register::Register,
        schema::{Schema, SchemaId},
        variant::{AccessQualifier, Inert, Variant},
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
        model.add_variant(qux, variant("A", 0));
        model.add_variant(qux, write(variant("A", 1)));
        model.add_field(
            bar,
            Field {
                access_modality: spanned(access_modality::ReadWrite),
                ..field("corge", (2, 2), Some(qux))
            },
        );

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
        model.add_variant(qux, variant("A", 0));
        let corge = model.add_schema(schema(None));
        let grault = model.add_schema(schema(None));
        model.add_variant(grault, variant("B", 0));
        model.add_field(bar, field("waldo", (0, 0), Some(baz)));
        model.add_field(bar, field("garply", (1, 1), Some(corge)));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (Structure::EmptySchema.code(), "assumed schema `baz` has no variants"),
                (Structure::EmptySchema.code(), "inherent schema of field `garply` has no variants"),
                (Structure::UnusedSchema.code(), "schema `qux` is never used"),
                (Structure::UnusedSchema.code(), "inherent schema is never used"),
                (
                    Structure::NoReadableVariants.code(),
                    "field `waldo` supports read access but exposes no readable variants",
                ),
                (
                    Structure::NoReadableVariants.code(),
                    "field `garply` supports read access but exposes no readable variants",
                ),
                (
                    Structure::NoWritableVariants.code(),
                    "field `waldo` supports write access but exposes no writable variants",
                ),
                (
                    Structure::NoWritableVariants.code(),
                    "field `garply` supports write access but exposes no writable variants",
                ),
            ],
            found: summary(&diagnostics),
            "expected an empty schema diagnostic for assumed schema `baz` and for the inherent schema of field \
             `garply`, an unused schema diagnostic for `qux` and for an inherent schema exposed by no field, and \
             missing variant diagnostics for the store fields `waldo` and `garply` exposing empty schemas",
        );
    }

    #[test]
    fn variant_overlaps() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_field(
            bar,
            Field {
                access_modality: spanned(access_modality::ReadWrite),
                ..field("qux", (2, 0), Some(baz))
            },
        );

        model.add_variant(baz, variant("A", 0));
        model.add_variant(baz, variant("B", 0));
        model.add_variant(baz, read(variant("C", 1)));
        model.add_variant(baz, write(variant("D", 1)));
        model.add_variant(baz, read(variant("E", 2)));
        model.add_variant(baz, write(variant("F", 2)));
        model.add_variant(baz, variant("G", 2));
        model.add_variant(baz, read(variant("H", 3)));
        model.add_variant(baz, read(variant("I", 3)));
        model.add_variant(baz, write(variant("J", 3)));
        model.add_variant(baz, variant("K", 4));
        model.add_variant(baz, variant("L", 4));
        model.add_variant(baz, read(variant("M", 4)));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    "variants `A` and `B` overlap, occupying discriminant `0x0`",
                    vec!["`A` assigned here", "`B` assigned here"],
                ),
                (
                    "variants `E` and `G` overlap, occupying read discriminant `0x2`",
                    vec!["`E` assigned here", "`G` assigned here"],
                ),
                (
                    "variants `H` and `I` overlap, occupying read discriminant `0x3`",
                    vec!["`H` assigned here", "`I` assigned here"],
                ),
                (
                    "variants `K`, `L`, and `M` overlap, occupying read discriminant `0x4`",
                    vec!["`K` assigned here", "`L` assigned here", "`M` assigned here"],
                ),
                (
                    "variants `F` and `G` overlap, occupying write discriminant `0x2`",
                    vec!["`F` assigned here", "`G` assigned here"],
                ),
                (
                    "variants `K` and `L` overlap, occupying write discriminant `0x4`",
                    vec!["`K` assigned here", "`L` assigned here"],
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        iter::once(&diagnostic.primary_label)
                            .chain(&diagnostic.supporting_labels)
                            .map(|label| label.message.as_str())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a diagnostic to be emitted for each read and write discriminant at which variants defined by \
             schema `baz` overlap, naming the access space unless the same variants overlap in both access spaces",
        );
    }

    #[test]
    fn inert_read_variants() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, inert(variant("A", 0)));
        model.add_variant(baz, read(inert(variant("B", 1))));
        model.add_variant(baz, read(variant("C", 2)));
        model.add_variant(baz, write(inert(variant("D", 3))));
        model.add_field(
            bar,
            Field {
                access_modality: spanned(access_modality::ReadWrite),
                ..field("qux", (1, 0), Some(baz))
            },
        );

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![(Structure::InertReadVariant.code(), "read variant `B` cannot be inert")],
            found: summary(&diagnostics),
            "expected a diagnostic to be emitted for the inert read variant `B` alone",
        );
    }

    #[test]
    fn out_of_range_discriminants() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, variant("A", 3));
        model.add_variant(baz, variant("B", 4));
        model.add_field(bar, field("qux", (1, 0), Some(baz)));
        model.add_field(bar, field("waldo", (4, 2), Some(baz)));

        let corge = model.add_register(foo, register("corge", 0x4));
        let grault = model.add_schema(schema(Some("grault")));
        model.add_variant(grault, variant("C", 0x1_0000_0000));
        model.add_field(corge, field("garply", (31, 0), Some(grault)));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: (
                vec![
                    (
                        Structure::DiscriminantOutOfRange.code(),
                        "variant `B` with discriminant `0x4` exceeds field `qux` of width `2`",
                    ),
                    (
                        Structure::DiscriminantOutOfRange.code(),
                        "variant `C` with discriminant `0x1_0000_0000` exceeds field `garply` of width `32`",
                    ),
                ],
                vec![
                    "discriminant `0x4` does not satisfy `discriminant <= 0x3`",
                    "discriminant `0x1_0000_0000` does not satisfy `discriminant <= 0xffff_ffff`",
                ],
            ),
            found: (
                summary(&diagnostics),
                diagnostics
                    .iter()
                    .flat_map(|diagnostic| &diagnostic.notes)
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            "expected a diagnostic to be emitted for variant `B` of shared schema `baz` exceeding field `qux` but not \
             `waldo`, and for variant `C` exceeding field `garply`",
        );
    }

    #[test]
    fn inexposable_variants() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));

        for (name, modality, bit, variants) in [
            (
                "qux",
                Read,
                0,
                vec![
                    variant("A", 0),
                    write(variant("B", 1)),
                    inert(variant("C", 2)),
                ],
            ),
            (
                "garply",
                Write,
                2,
                vec![read(variant("D", 0)), variant("I", 1)],
            ),
            (
                "corge",
                Store,
                4,
                vec![
                    read(variant("E", 0)),
                    write(variant("F", 1)),
                    inert(variant("J", 2)),
                ],
            ),
            (
                "grault",
                ReadWrite,
                6,
                vec![read(variant("G", 0)), write(variant("H", 1))],
            ),
        ] {
            let schema = model.add_schema(schema(None));

            for variant in variants {
                model.add_variant(schema, variant);
            }

            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    ..field(name, (bit + 1, bit), Some(schema))
                },
            );
        }

        let diagnostics = model.validate();

        let read_note =
            "variants that imply write access cannot be exposed by fields with modality `read`";
        let write_note =
            "variants that imply read access cannot be exposed by fields with modality `write`";
        let store_note =
            "access-specific variants cannot be exposed by fields with modality `store`";
        let store_inert_note = "inert variants cannot be exposed by fields with modality `store`";

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    "write variant `B` cannot be exposed by field `qux` with modality `read`",
                    "write qualifier",
                    vec![read_note],
                ),
                (
                    "inert variant `C` cannot be exposed by field `qux` with modality `read`",
                    "inert qualifier",
                    vec![read_note],
                ),
                (
                    "read variant `D` cannot be exposed by field `garply` with modality `write`",
                    "read qualifier",
                    vec![write_note],
                ),
                (
                    "read variant `E` cannot be exposed by field `corge` with modality `store`",
                    "read qualifier",
                    vec![store_note],
                ),
                (
                    "write variant `F` cannot be exposed by field `corge` with modality `store`",
                    "write qualifier",
                    vec![store_note],
                ),
                (
                    "inert variant `J` cannot be exposed by field `corge` with modality `store`",
                    "inert qualifier",
                    vec![store_inert_note],
                ),
            ],
            found: diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.is(Structure::InexposableVariant))
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        diagnostic.primary_label.message.as_str(),
                        diagnostic.notes.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a diagnostic to be emitted for each variant of the inherent schemas of fields \
             `qux`, `garply`, and `corge` that the field cannot expose, labelling the responsible qualifier, and \
             none for field `grault`",
        );
    }

    #[test]
    fn incompatible_fields() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, read(variant("A", 0)));
        model.add_variant(baz, read(variant("B", 1)));
        model.add_variant(baz, write(variant("C", 1)));
        model.add_variant(baz, inert(variant("D", 2)));

        for (name, modality, bit) in [("qux", Read, 0), ("corge", ReadWrite, 2)] {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    ..field(name, (bit + 1, bit), Some(baz))
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![(
                "field `qux` with modality `read` cannot assume schema `baz`",
                "incompatible schema",
                vec![
                    ("write variant `C` cannot be exposed by field `qux` with modality `read`", "write qualifier"),
                    ("inert variant `D` cannot be exposed by field `qux` with modality `read`", "inert qualifier"),
                ],
            )],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        diagnostic.primary_label.message.as_str(),
                        diagnostic
                            .children
                            .iter()
                            .map(|child| (child.message.as_str(), child.primary_label.message.as_str()))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected one diagnostic to be emitted for field `qux` assuming schema `baz`, containing a child for each \
             variant defined by `baz` that `qux` cannot expose, and none for field `corge`",
        );
    }

    #[test]
    fn missing_variants() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, write(variant("A", 0)));
        let qux = model.add_schema(schema(Some("qux")));
        model.add_variant(qux, read(variant("B", 0)));
        let corge = model.add_schema(schema(None));
        model.add_variant(corge, write(variant("C", 0)));

        for (name, modality, schema, bit) in [
            ("grault", ReadWrite, baz, 0),
            ("garply", Write, baz, 1),
            ("waldo", Read, qux, 2),
            ("fred", ReadWrite, qux, 3),
            ("plugh", ReadWrite, corge, 4),
        ] {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    ..field(name, (bit, bit), Some(schema))
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Severity::Warning,
                    "field `grault` supports read access but exposes no readable variants",
                    vec!["assumed schema `baz` defines no readable variants"],
                ),
                (
                    Severity::Warning,
                    "field `plugh` supports read access but exposes no readable variants",
                    vec![],
                ),
                (
                    Severity::Warning,
                    "field `fred` supports write access but exposes no writable variants",
                    vec!["assumed schema `qux` defines no writable variants"],
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.id.severity,
                        diagnostic.message.as_str(),
                        diagnostic.notes.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a warning to be emitted for each field supporting an access but exposing no variants \
             participating in that access, noting the assumed schema where there is one, and none for `garply` and \
             `waldo`",
        );
    }

    #[test]
    fn out_of_range_resets() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let corge = model.add_register(foo, register("corge", 0x4));

        for (register, name, domain, value) in [
            (bar, "qux", (1, 0), 0x4),
            (bar, "grault", (3, 2), 0x3),
            (corge, "garply", (31, 0), 0x1_0000_0000),
        ] {
            model.add_field(
                register,
                Field {
                    access_modality: spanned(access_modality::Store),
                    reset: Some(spanned(value)),
                    ..field(name, domain, None)
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: (
                vec![
                    (Structure::ResetOutOfRange.code(), "reset `0x4` exceeds field `qux` of width `2`"),
                    (
                        Structure::ResetOutOfRange.code(),
                        "reset `0x1_0000_0000` exceeds field `garply` of width `32`",
                    ),
                ],
                vec![
                    "reset `0x4` does not satisfy `reset <= 0x3`",
                    "reset `0x1_0000_0000` does not satisfy `reset <= 0xffff_ffff`",
                ],
            ),
            found: (
                summary(&diagnostics),
                diagnostics
                    .iter()
                    .flat_map(|diagnostic| &diagnostic.notes)
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            "expected a diagnostic to be emitted for the resets of fields `qux` and `garply` exceeding the field, and \
             none for field `grault`",
        );
    }

    #[test]
    fn ignored_resets() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));

        for (name, modality, bit) in [
            ("qux", Read, 0),
            ("waldo", Write, 2),
            ("corge", ReadWrite, 4),
            ("grault", Store, 6),
            ("garply", VolatileStore, 8),
        ] {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    reset: Some(spanned(0x1)),
                    ..field(name, (bit + 1, bit), None)
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Severity::Warning,
                    Structure::IgnoredReset.code(),
                    "reset of field `qux` with modality `read` is ignored",
                ),
                (
                    Severity::Warning,
                    Structure::IgnoredReset.code(),
                    "reset of field `waldo` with modality `write` is ignored",
                ),
                (
                    Severity::Warning,
                    Structure::IgnoredReset.code(),
                    "reset of field `corge` with modality `read write` is ignored",
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.id.severity, diagnostic.id.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>(),
            "expected a warning to be emitted for the resets of fields `qux`, `waldo`, and `corge`, and none for the \
             store fields `grault` and `garply`",
        );
    }

    #[test]
    fn uninhabitable_resets() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, variant("A", 0));
        model.add_variant(baz, variant("B", 1));
        let thud = model.add_schema(schema(Some("thud")));
        model.add_variant(thud, variant("C", 0));

        for (name, modality, bit, schema, reset) in [
            ("qux", Store, 0, Some(baz), 0x2),
            ("fred", VolatileStore, 2, Some(baz), 0x1),
            ("corge", Store, 4, None, 0x3),
            ("grault", Store, 6, Some(baz), 0x0),
            ("garply", Read, 8, Some(baz), 0x3),
            ("waldo", Store, 10, Some(baz), 0x4),
            ("xyzzy", Store, 12, Some(thud), 0x0),
        ] {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    reset: Some(spanned(reset)),
                    ..field(name, (bit + 1, bit), schema)
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Structure::IgnoredReset.code(),
                    "reset of field `garply` with modality `read` is ignored",
                ),
                (
                    Structure::UninhabitableReset.code(),
                    "reset `0x2` has no corresponding variant exposed by field `qux`",
                ),
                (
                    Structure::UninhabitableReset.code(),
                    "reset `0x4` has no corresponding variant exposed by field `waldo`",
                ),
            ],
            found: summary(&diagnostics),
            "expected a diagnostic to be emitted for the resets of the enumerated store fields `qux` and `waldo` \
             identifying no variant of `baz`, and none for the numeric field `corge` or the resets of `grault` and \
             `xyzzy`, which identify variants `A` and `C`",
        );
    }

    #[test]
    fn missing_resets() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));

        for (name, modality, bit, reset) in [
            ("qux", Store, 0, None),
            ("garply", VolatileStore, 2, None),
            ("corge", Store, 4, Some(0x1)),
            ("grault", Read, 6, None),
        ] {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    reset: reset.map(spanned),
                    ..field(name, (bit + 1, bit), None)
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: (
                vec![
                    (Structure::MissingReset.code(), "field `qux` with modality `store` has no reset"),
                    (
                        Structure::MissingReset.code(),
                        "field `garply` with modality `volatile store` has no reset",
                    ),
                ],
                vec![
                    "resets define the statically known initial state of fields with modality `store`, which is required",
                    "resets define the statically known initial state of fields with modality `volatile store`, which is required",
                ],
            ),
            found: (
                summary(&diagnostics),
                diagnostics
                    .iter()
                    .flat_map(|diagnostic| &diagnostic.notes)
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            ),
            "expected a diagnostic to be emitted for the store fields `qux` and `garply` without a reset, and none for \
             the store field `corge` defining a reset or the field `grault` with modality `read`",
        );
    }

    #[test]
    fn uninhabitable_entitlements() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, variant("A", 0));
        let thud = model.add_schema(schema(Some("thud")));
        model.add_variant(thud, variant("B", 0));

        let [qux, garply, corge] = [
            ("qux", (1, 0), Some(baz), 0x0),
            ("garply", (3, 2), Some(thud), 0x0),
            ("corge", (7, 4), None, 0x0),
        ]
        .map(|(name, domain, schema, reset)| {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(access_modality::Store),
                    reset: Some(spanned(reset)),
                    ..field(name, domain, schema)
                },
            )
        });

        model.add_field(
            bar,
            Field {
                ontological_entitlements: Some(space([
                    Entitlement {
                        field: qux,
                        state: 0x0,
                    },
                    Entitlement {
                        field: qux,
                        state: 0x1,
                    },
                    Entitlement {
                        field: corge,
                        state: 0xf,
                    },
                    Entitlement {
                        field: corge,
                        state: 0x10,
                    },
                ])),
                ..field("grault", (8, 8), None)
            },
        );

        model[foo].ontological_entitlements = Some(space([Entitlement {
            field: garply,
            state: 0x0,
        }]));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Structure::UninhabitableEntitlement.code(),
                    "entitlement value `0x1` has no corresponding variant exposed by field `qux`",
                ),
                (
                    Structure::EntitlementOutOfRange.code(),
                    "entitlement value `0x10` exceeds field `corge` of width `4`",
                ),
            ],
            found: summary(&diagnostics),
            "expected a diagnostic to be emitted for each entitlement to a value identifying no variant of an \
             enumerated field, or exceeding the domain of a numeric field, and none for the entitlements of `qux` and \
             `garply` to `0x0`, which identify variants `A` and `B`, or of `corge` to `0xf`",
        );
    }

    #[test]
    fn access_specific_states() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        model.add_variant(baz, variant("A", 0));
        model.add_variant(baz, read(variant("E", 2)));

        let qux = model.add_field(
            bar,
            Field {
                access_modality: spanned(access_modality::Store),
                reset: Some(spanned(0x2)),
                ..field("qux", (1, 0), Some(baz))
            },
        );

        model.add_field(
            bar,
            Field {
                ontological_entitlements: Some(space([Entitlement {
                    field: qux,
                    state: 0x2,
                }])),
                ..field("corge", (2, 2), None)
            },
        );

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Structure::IncompatibleField.code(),
                    "field `qux` with modality `store` cannot assume schema `baz`",
                ),
                (
                    Structure::UninhabitableReset.code(),
                    "reset `0x2` has no corresponding variant exposed by field `qux`",
                ),
                (
                    Structure::UninhabitableEntitlement.code(),
                    "entitlement value `0x2` has no corresponding variant exposed by field `qux`",
                ),
            ],
            found: summary(&diagnostics),
            "expected the reset of field `qux` and the entitlement of `corge` to `0x2`, which identifies only the read \
             variant `E`, to each have no corresponding variant exposed by `qux`, in addition to the diagnostic of \
             `qux` with modality `store` being incompatible with `baz`",
        );
    }

    #[test]
    fn unresolvable_entitlements() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));

        let [qux, waldo, corge, grault] = [
            ("qux", Read, 0, None),
            ("waldo", Store, 1, Some(0x0)),
            ("corge", VolatileStore, 2, Some(0x0)),
            ("grault", VolatileStore, 3, Some(0x0)),
        ]
        .map(|(name, modality, bit, reset)| {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    reset: reset.map(spanned),
                    ..field(name, (bit, bit), None)
                },
            )
        });

        model[grault].hardware_affordance_entitlements = Some(space([Entitlement {
            field: waldo,
            state: 0x1,
        }]));

        model.add_field(
            bar,
            Field {
                ontological_entitlements: Some(space(
                    [qux, waldo, corge, grault].map(|field| Entitlement { field, state: 0x0 }),
                )),
                ..field("garply", (4, 4), None)
            },
        );

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    "entitlement targets unresolvable field `qux` with modality `read`",
                    vec!["only resolvable fields inhabit states"],
                ),
                (
                    "entitlement targets unresolvable field `corge` with modality `volatile store`",
                    vec![
                        "only resolvable fields inhabit states",
                        "fields with modality `volatile store` are resolvable only when hardware affordance \
                         entitlements are non-tautological",
                    ],
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        diagnostic.notes.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a diagnostic to be emitted for the entitlements imposed by `garply` to the unresolvable fields \
             `qux` and `corge`, and none for the store field `waldo` or field `grault` with modality `volatile store`, \
             which imposes a hardware affordance entitlement space",
        );
    }

    #[test]
    fn unpermitted_entitlement_spaces() {
        use access_modality::*;

        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let waldo = model.add_field(
            bar,
            Field {
                access_modality: spanned(Store),
                reset: Some(spanned(0x0)),
                ..field("waldo", (0, 0), None)
            },
        );

        for (name, modality, bit, affordance, hardware_affordance) in [
            ("qux", Read, 1, true, false),
            ("fred", Write, 2, true, false),
            ("corge", Store, 3, false, true),
            ("grault", VolatileStore, 4, true, true),
            ("garply", Read, 5, false, true),
        ] {
            let space = || {
                space([Entitlement {
                    field: waldo,
                    state: 0x0,
                }])
            };

            model.add_field(
                bar,
                Field {
                    access_modality: spanned(modality),
                    reset: modality.is_store().then(|| spanned(0x0)),
                    affordance_entitlements: affordance.then(space),
                    hardware_affordance_entitlements: hardware_affordance.then(space),
                    ..field(name, (bit, bit), None)
                },
            );
        }

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    "field `qux` with modality `read` cannot impose affordance entitlements",
                    vec!["affordance entitlements constrain software writes, which fields with modality `read` do not \
                          support"],
                ),
                (
                    "field `corge` with modality `store` cannot impose hardware affordance entitlements",
                    vec!["hardware affordance entitlements only serve to determine whether fields with modality \
                          `volatile store` are resolvable"],
                ),
                (
                    "field `garply` with modality `read` cannot impose hardware affordance entitlements",
                    vec!["hardware affordance entitlements only serve to determine whether fields with modality \
                          `volatile store` are resolvable"],
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        diagnostic.notes.iter().map(String::as_str).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a diagnostic to be emitted for the affordance entitlements of field `qux` with modality `read` \
             and the hardware affordance entitlements of the fields `corge` and `garply`, and none for `fred` or \
             `grault`",
        );
    }

    #[test]
    fn foreign_statewise_variants() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        let baz = model.add_schema(schema(Some("baz")));
        let a = model.add_variant(baz, variant("A", 0));
        let thud = model.add_schema(schema(Some("thud")));
        let b = model.add_variant(thud, variant("B", 0));

        let [qux, grault, corge] = [
            ("qux", (0, 0), Some(baz), 0x0),
            ("grault", (1, 1), Some(thud), 0x0),
            ("corge", (2, 2), None, 0x0),
        ]
        .map(|(name, domain, schema, reset)| {
            model.add_field(
                bar,
                Field {
                    access_modality: spanned(access_modality::Store),
                    reset: Some(spanned(reset)),
                    ..field(name, domain, schema)
                },
            )
        });

        let space = || {
            space([Entitlement {
                field: corge,
                state: 0x0,
            }])
        };

        model[qux].statewise_entitlements = [(a, space()), (b, space())].into();
        model[grault].statewise_entitlements = [(b, space())].into();
        model[corge].statewise_entitlements = [(a, space())].into();

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    Structure::ForeignVariant.code(),
                    "statewise entitlement variant `B` is foreign to field `qux`",
                ),
                (
                    Structure::ForeignVariant.code(),
                    "statewise entitlement variant `A` is foreign to field `corge`",
                ),
            ],
            found: summary(&diagnostics),
            "expected a diagnostic to be emitted for the statewise entitlements imposed by variants foreign to the \
             containing field, `B` within `qux` and `A` within the numeric field `corge`, and none for `A` within \
             `qux` or `B` within `grault`",
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
        model.add_register(foo, register("waldo", 0x4));
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
                    "peripherals `foo` and `bar` overlap, occupying bytes `0x0000_0004..0x0000_0008`",
                ),
                (
                    Structure::RegisterOverlap.code(),
                    "registers `qux` and `waldo` overlap, occupying bytes `0x0000_0004..0x0000_0008`",
                ),
                (Structure::FieldOverlap.code(), "fields `a` and `b` overlap, occupying bits `2:2`"),
                (Structure::FieldOverlap.code(), "fields `a`, `b`, and `d` overlap, occupying bits `3:3`"),
                (Structure::FieldOverlap.code(), "fields `b` and `d` overlap, occupying bits `4:4`"),
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
                "register `foo::waldo` overlaps",
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
    fn overlap_participants() {
        let mut model = Model::new(spanned(4));

        let foo = model.add_peripheral(peripheral("foo", 0x0));
        let bar = model.add_register(foo, register("bar", 0x0));
        model.add_field(bar, field("a", (7, 5), None));
        model.add_field(bar, field("b", (6, 0), None));
        model.add_field(bar, field("c", (5, 2), None));
        model.add_field(bar, field("d", (5, 5), None));

        let diagnostics = model.validate();

        similar_asserts::assert_eq!(
            expected: vec![
                (
                    "fields `b` and `c` overlap, occupying bits `4:2`",
                    vec!["`b` placed here", "`c` placed here"],
                ),
                (
                    "fields `a`, `b`, and 2 others overlap, occupying bits `5:5`",
                    vec!["`a` placed here", "`b` placed here", "`c` placed here", "`d` placed here"],
                ),
                (
                    "fields `a` and `b` overlap, occupying bits `6:6`",
                    vec!["`a` placed here", "`b` placed here"],
                ),
            ],
            found: diagnostics
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.message.as_str(),
                        iter::once(&diagnostic.primary_label)
                            .chain(&diagnostic.supporting_labels)
                            .map(|label| label.message.as_str())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
            "expected a diagnostic to be emitted for each region of register `bar` occupied by more than one field, \
             naming and labelling the fields in the order they were added",
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
                    "registers `bar` and `baz` overlap, occupying bytes `0x0003..0x0004`",
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
            ontological_entitlements: None,
            docs: Vec::new(),
            leaky: None,
            span: SPAN,
        }
    }

    fn register(name: &str, offset: Word) -> Register {
        Register {
            name: spanned(name.to_string()),
            offset: spanned(offset),
            access: Vec::new(),
            docs: Vec::new(),
            leaky: None,
            span: SPAN,
        }
    }

    fn field(name: &str, (msb, lsb): (u8, u8), schema: Option<SchemaId>) -> Field {
        Field {
            name: spanned(name.to_string()),
            domain: spanned(Domain::new(msb, lsb).unwrap()),
            access_modality: spanned(access_modality::ReadWrite),
            schema: schema.map(spanned),
            reset: None,
            ontological_entitlements: None,
            affordance_entitlements: None,
            hardware_affordance_entitlements: None,
            statewise_entitlements: BTreeMap::new(),
            docs: Vec::new(),
            leaky: None,
            span: SPAN,
        }
    }

    /// Create an entitlement space with a single pattern listing the provided entitlements.
    fn space(entitlements: impl IntoIterator<Item = Entitlement>) -> Spanned<Space> {
        spanned(Space {
            patterns: vec![spanned(Pattern {
                entitlements: entitlements.into_iter().map(spanned).collect(),
            })],
        })
    }

    fn schema(name: Option<&str>) -> Schema {
        Schema {
            name: name.map(|name| spanned(name.to_string())),
            docs: Vec::new(),
            span: SPAN,
        }
    }

    fn variant(name: &str, discriminant: Word) -> Variant {
        Variant {
            name: spanned(name.to_string()),
            discriminant: spanned(discriminant),
            inert: None,
            access: None,
            docs: Vec::new(),
            span: SPAN,
        }
    }

    fn inert(variant: Variant) -> Variant {
        Variant {
            inert: Some(spanned(Inert)),
            ..variant
        }
    }

    fn read(variant: Variant) -> Variant {
        Variant {
            access: Some(spanned(AccessQualifier::Read)),
            ..variant
        }
    }

    fn write(variant: Variant) -> Variant {
        Variant {
            access: Some(spanned(AccessQualifier::Write)),
            ..variant
        }
    }
}
