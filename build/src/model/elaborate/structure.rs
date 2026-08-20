//! Phase one: structure — walking the device into declaration trees.

use ::model::{
    Composition,
    decl::{FieldDecl, GroupDecl, PeripheralDecl, RegisterDecl, SchemaDecl, VariantDecl},
    variant::ParentIndex,
};
use heck::ToSnakeCase as _;
use proc_macro2::Span as IdentSpan;
use syn::Ident;
use syntax::ast::{
    Device, DeviceItem, Field, FieldItem, GroupItem, InterruptKind, Interrupts, ListEntry,
    Peripheral, PeripheralItem, Register, RegisterItem, ResetValue, Schema, Span, Spanned,
    ValueEntry, VariantValue,
};

use crate::model::semantic::Diagnostic;

use super::{
    Context, DeclaredVariant, Enclosing, Link, Location, Locator, Modality, Pending, PlacedSchema,
    Placement, Reference, Target, VariantPath, corresponding, expand, head_span, ident, merge,
    modality_access, side_decl,
};

impl<'ast, 'src> Context<'ast, 'src> {
    pub(super) fn device(
        &mut self,
        composition: &mut Composition,
        device: &Device<'src>,
        span: Span,
    ) {
        let Some(body) = &device.body else {
            self.diagnostics.push(Diagnostic::empty_device(span));
            return;
        };

        for item in &body.inner {
            match &item.inner {
                DeviceItem::Peripheral(peripheral) => {
                    for declaration in self.peripheral_decls(peripheral, item.span, &[]) {
                        composition.insert_peripheral(declaration);
                    }
                }
                DeviceItem::PeripheralGroup(group) => {
                    let Some(name) = &group.name else {
                        self.diagnostics
                            .push(Diagnostic::unnamed(item.span, "group"));
                        continue;
                    };

                    let prefix = vec![name.inner.to_string()];
                    let mut members = Vec::new();

                    for member in &group.body.inner {
                        match &member.inner {
                            GroupItem::Member(peripheral) => {
                                members.extend(self.peripheral_decls(
                                    peripheral,
                                    member.span,
                                    &prefix,
                                ));
                            }
                            GroupItem::Schema(schema) => {
                                self.place(
                                    schema,
                                    member.span,
                                    prefix.clone(),
                                    Locator::PeripheralGroup(name.inner.to_string()),
                                );
                            }
                            GroupItem::Error => {}
                        }
                    }

                    composition.insert_peripheral_group(GroupDecl {
                        name: name.inner.to_string(),
                        members,
                    });
                }
                DeviceItem::Schema(schema) => {
                    self.place(schema, item.span, Vec::new(), Locator::Root);
                }
                DeviceItem::Interrupts(interrupts) => {
                    self.interrupts(composition, interrupts);
                }
                DeviceItem::Error => {}
            }
        }
    }

    /// The declarations a peripheral definition describes: one per element.
    pub(super) fn peripheral_decls(
        &mut self,
        peripheral: &Peripheral<'src>,
        span: Span,
        prefix: &[String],
    ) -> Vec<PeripheralDecl> {
        let Some(peripheral) = self.resolve_peripheral(peripheral) else {
            return Vec::new();
        };

        let Some(elements) = self.scalar_elements(
            &peripheral.head.name,
            peripheral.array,
            &peripheral.head.indices,
            &peripheral.domain,
            Err("peripheral addresses cannot be auto-incremented — list every address explicitly"),
            "peripheral",
            span,
            0,
        ) else {
            return Vec::new();
        };

        let mut out = Vec::new();

        let head = head_span(
            peripheral.head.name.as_ref(),
            span,
            &[
                peripheral.head.indices.as_ref().map(|indices| indices.span),
                peripheral.domain.as_ref().map(|domain| domain.span),
                peripheral.requires.as_ref().map(|requires| requires.span),
            ],
        );

        let length = elements.len();

        for (ordinal, (name, base)) in elements.into_iter().enumerate() {
            // the array definitions enclosing everything within this element
            let enclosing = peripheral
                .head
                .indices
                .as_ref()
                .map(|indices| Enclosing {
                    ordinal,
                    length,
                    span: indices.span,
                })
                .into_iter()
                .collect::<Vec<_>>();

            self.locations.insert(
                vec![name.to_snake_case()],
                Location {
                    head,
                    domain: peripheral.domain.as_ref().map(|domain| domain.span),
                },
            );

            let mut composed = ::model::Peripheral::new(&name, base)
                .docs(peripheral.docs.iter().map(|doc| doc.inner));

            if peripheral.leaky.is_some() {
                composed = composed.leaky();
            }

            if let Some(requires) = &peripheral.requires {
                self.pending.push(Pending {
                    target: Target::Peripheral(name.clone()),
                    space: requires.clone(),
                    arrays: enclosing.clone(),
                });
            }

            let mut registers = Vec::new();
            let mut register_groups = Vec::new();

            if let Some(body) = &peripheral.body {
                let mut path = prefix.to_vec();
                path.push(name.clone());

                self.register_items(
                    &body.inner,
                    &path,
                    &name,
                    &enclosing,
                    &mut registers,
                    &mut register_groups,
                );
            }

            out.push(PeripheralDecl {
                peripheral: composed,
                registers,
                register_groups,
            });
        }

        out
    }

    /// Collect a peripheral body's registers and register groups.
    pub(super) fn register_items(
        &mut self,
        items: &[Spanned<PeripheralItem<'src>>],
        path: &[String],
        peripheral: &str,
        enclosing: &[Enclosing],
        registers: &mut Vec<RegisterDecl>,
        groups: &mut Vec<GroupDecl<RegisterDecl>>,
    ) {
        for item in items {
            match &item.inner {
                PeripheralItem::Register(register) => {
                    registers.extend(
                        self.register_decls(register, item.span, path, peripheral, enclosing),
                    );
                }
                PeripheralItem::RegisterGroup(group) => {
                    let Some(name) = &group.name else {
                        self.diagnostics
                            .push(Diagnostic::unnamed(item.span, "group"));
                        continue;
                    };

                    let mut group_path = path.to_vec();
                    group_path.push(name.inner.to_string());

                    let mut members = Vec::new();

                    for member in &group.body.inner {
                        match &member.inner {
                            GroupItem::Member(register) => {
                                members.extend(self.register_decls(
                                    register,
                                    member.span,
                                    &group_path,
                                    peripheral,
                                    enclosing,
                                ));
                            }
                            GroupItem::Schema(schema) => {
                                self.place(
                                    schema,
                                    member.span,
                                    group_path.clone(),
                                    Locator::RegisterGroup(name.inner.to_string()),
                                );
                            }
                            GroupItem::Error => {}
                        }
                    }

                    groups.push(GroupDecl {
                        name: name.inner.to_string(),
                        members,
                    });
                }
                PeripheralItem::Schema(schema) => {
                    self.place(
                        schema,
                        item.span,
                        path.to_vec(),
                        Locator::Peripheral(peripheral.to_string()),
                    );
                }
                PeripheralItem::Error => {}
            }
        }
    }

    /// The declarations a register definition describes: one per element.
    pub(super) fn register_decls(
        &mut self,
        register: &Register<'src>,
        span: Span,
        prefix: &[String],
        peripheral: &str,
        enclosing: &[Enclosing],
    ) -> Vec<RegisterDecl> {
        let Some(register) = self.resolve_register(register) else {
            return Vec::new();
        };

        let reset = match &register.reset {
            None => None,
            Some(reset) => match reset.inner {
                ResetValue::Value(value) => Some(value),
                ResetValue::Variant(..) => {
                    self.diagnostics
                        .push(Diagnostic::non_numeric_register_reset(reset.span));
                    None
                }
            },
        };

        let Some(elements) = self.scalar_elements(
            &register.head.name,
            register.array,
            &register.head.indices,
            &register.domain,
            Ok(4),
            "register",
            span,
            enclosing.last().map(|array| array.ordinal).unwrap_or(0),
        ) else {
            return Vec::new();
        };

        let mut out = Vec::new();

        let head = head_span(
            register.head.name.as_ref(),
            span,
            &[
                register.head.indices.as_ref().map(|indices| indices.span),
                register.domain.as_ref().map(|domain| domain.span),
                register.reset.as_ref().map(|reset| reset.span),
            ],
        );

        let length = elements.len();

        for (ordinal, (name, offset)) in elements.into_iter().enumerate() {
            let mut inner = enclosing.to_vec();

            if let Some(indices) = &register.head.indices {
                inner.push(Enclosing {
                    ordinal,
                    length,
                    span: indices.span,
                });
            }

            self.locations.insert(
                vec![peripheral.to_snake_case(), name.to_snake_case()],
                Location {
                    head,
                    domain: register.domain.as_ref().map(|domain| domain.span),
                },
            );

            let mut composed = ::model::Register::new(&name, offset)
                .docs(register.docs.iter().map(|doc| doc.inner));

            if let Some(reset) = reset {
                composed = composed.reset(reset);
            }

            if register.leaky.is_some() {
                composed = composed.leaky();
            }

            let mut fields = Vec::new();
            let mut field_groups = Vec::new();

            if let Some(body) = &register.body {
                let mut path = prefix.to_vec();
                path.push(name.clone());

                self.field_items(
                    &body.inner,
                    &path,
                    peripheral,
                    &name,
                    &inner,
                    &mut fields,
                    &mut field_groups,
                );
            }

            out.push(RegisterDecl {
                register: composed,
                fields,
                field_groups,
            });
        }

        out
    }

    /// Collect a register body's fields and field groups.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn field_items(
        &mut self,
        items: &[Spanned<RegisterItem<'src>>],
        path: &[String],
        peripheral: &str,
        register: &str,
        enclosing: &[Enclosing],
        fields: &mut Vec<FieldDecl>,
        groups: &mut Vec<GroupDecl<FieldDecl>>,
    ) {
        for item in items {
            match &item.inner {
                RegisterItem::Field(field) => {
                    fields.extend(
                        self.field_decls(field, item.span, path, peripheral, register, enclosing),
                    );
                }
                RegisterItem::FieldGroup(group) => {
                    let Some(name) = &group.name else {
                        self.diagnostics
                            .push(Diagnostic::unnamed(item.span, "group"));
                        continue;
                    };

                    let mut group_path = path.to_vec();
                    group_path.push(name.inner.to_string());

                    let mut members = Vec::new();

                    for member in &group.body.inner {
                        match &member.inner {
                            GroupItem::Member(field) => {
                                members.extend(self.field_decls(
                                    field,
                                    member.span,
                                    &group_path,
                                    peripheral,
                                    register,
                                    enclosing,
                                ));
                            }
                            GroupItem::Schema(schema) => {
                                self.place(
                                    schema,
                                    member.span,
                                    group_path.clone(),
                                    Locator::FieldGroup(name.inner.to_string()),
                                );
                            }
                            GroupItem::Error => {}
                        }
                    }

                    groups.push(GroupDecl {
                        name: name.inner.to_string(),
                        members,
                    });
                }
                RegisterItem::Schema(schema) => {
                    self.place(
                        schema,
                        item.span,
                        path.to_vec(),
                        Locator::Register(peripheral.to_string(), register.to_string()),
                    );
                }
                RegisterItem::Error => {}
            }
        }
    }

    /// The declarations a field definition describes: one per element.
    ///
    /// Recording happens here too: every declared variant enters the
    /// resolution table, and every `requires` clause becomes a [`Pending`]
    /// registration for phase two.
    pub(super) fn field_decls(
        &mut self,
        field: &Field<'src>,
        span: Span,
        prefix: &[String],
        peripheral: &str,
        register: &str,
        enclosing: &[Enclosing],
    ) -> Vec<FieldDecl> {
        let field = match field.head.template.clone() {
            None => field.clone(),
            Some(template_ref) => {
                self.redundant_as(&field.head);

                let Some(template) = self.resolve_template(
                    &template_ref,
                    "field",
                    |templates, name| templates.fields.get(name).copied(),
                    |templates| {
                        templates
                            .fields
                            .keys()
                            .map(|name| name.to_string())
                            .collect()
                    },
                ) else {
                    return Vec::new();
                };

                merge::field(template.clone(), field)
            }
        };

        let field_name = field
            .head
            .name
            .as_ref()
            .map(|name| name.inner)
            .unwrap_or("_");

        if let (Some(assumes), [extends, ..]) = (&field.assumes, field.extends.as_slice()) {
            self.diagnostics.push(Diagnostic::assumes_and_extends(
                extends.span,
                field_name,
                assumes.span,
            ));
            return Vec::new();
        }

        let assumes = field.assumes.is_some();

        if let Some(reference) = &field.assumes
            && let Some(body) = &field.body
        {
            self.diagnostics.push(Diagnostic::assumed_variants(
                body.span,
                field_name,
                reference.span,
            ));
        }

        let Some(modality) = self.modality(&field.access, "field", span) else {
            return Vec::new();
        };

        // the field's access words, as one span — the "field is declared
        // {modality} here" party in side diagnostics
        let modality_span = {
            let first = field.access.first().expect("modality resolved").span;
            let last = field.access.last().expect("modality resolved").span;

            Span {
                start: first.start,
                end: last.end.max(first.end),
                context: first.context,
            }
        };

        if let Some(requires) = &field.requires.write
            && matches!(modality, Modality::Read)
        {
            self.diagnostics.push(Diagnostic::write_requires_readonly(
                requires.span,
                field_name,
            ));
        }

        if let Some(requires) = &field.requires.hardware_write
            && !matches!(modality, Modality::VolatileStore)
        {
            self.diagnostics.push(Diagnostic::hardware_write_requires(
                requires.span,
                field_name,
                modality.to_string(),
            ));
        }

        let Some(elements) = self.bit_elements(
            &field,
            span,
            enclosing.last().map(|array| array.ordinal).unwrap_or(0),
        ) else {
            return Vec::new();
        };

        let variants = if assumes {
            Vec::new()
        } else {
            self.variants_of(&field.body)
        };

        // every side an inline variant names, the field must have
        for declared in &variants {
            let Some(side) = &declared.side else {
                continue;
            };

            self.validate_side(&declared.name, side, modality, field_name, modality_span);
        }

        let access = modality_access(modality);

        let length = elements.len();

        let mut out = Vec::new();

        for (ordinal, (name, (offset, width))) in elements.into_iter().enumerate() {
            // the array definitions enclosing this element, outermost first
            let arrays = {
                let mut arrays = enclosing.to_vec();

                if let Some(indices) = &field.head.indices {
                    arrays.push(Enclosing {
                        ordinal,
                        length,
                        span: indices.span,
                    });
                }

                arrays
            };

            if offset >= 32 || width > 32 {
                self.diagnostics
                    .push(Diagnostic::does_not_fit(span, &name, offset, width));
                continue;
            }

            let mut composed = ::model::Field::new(&name, offset as u8, width as u8)
                .docs(field.docs.iter().map(|doc| doc.inner));

            if field.leaky.is_some() {
                composed = composed.leaky();
            }

            let chain = (peripheral.to_string(), register.to_string(), name.clone());

            self.locations.insert(
                vec![
                    peripheral.to_snake_case(),
                    register.to_snake_case(),
                    name.to_snake_case(),
                ],
                Location {
                    head: head_span(
                        field.head.name.as_ref(),
                        span,
                        &[
                            field.head.indices.as_ref().map(|indices| indices.span),
                            field.domain.as_ref().map(|domain| domain.span),
                            field.assumes.as_ref().map(|assumes| assumes.span),
                            field.extends.last().map(|extends| extends.span),
                            field.reset.as_ref().map(|reset| reset.span),
                            field
                                .requires
                                .inherent
                                .as_ref()
                                .map(|requires| requires.span),
                            field.requires.write.as_ref().map(|requires| requires.span),
                            field
                                .requires
                                .hardware_write
                                .as_ref()
                                .map(|requires| requires.span),
                        ],
                    ),
                    domain: field.domain.as_ref().map(|domain| domain.span),
                },
            );

            match &field.reset {
                None => {}
                Some(reset) => match reset.inner {
                    ResetValue::Value(value) => composed = composed.reset(value),
                    ResetValue::Variant(variant) => {
                        self.resets
                            .push((chain.clone(), variant.to_string(), reset.span));
                    }
                },
            }

            if let Some(requires) = &field.requires.inherent {
                self.pending.push(Pending {
                    target: Target::Field(chain.clone()),
                    space: requires.clone(),
                    arrays: arrays.clone(),
                });
            }

            if let Some(requires) = &field.requires.write
                && !matches!(modality, Modality::Read)
            {
                self.pending.push(Pending {
                    target: Target::Write(chain.clone()),
                    space: requires.clone(),
                    arrays: arrays.clone(),
                });
            }

            if let Some(requires) = &field.requires.hardware_write
                && matches!(modality, Modality::VolatileStore)
            {
                self.pending.push(Pending {
                    target: Target::HardwareWrite(chain.clone()),
                    space: requires.clone(),
                    arrays: arrays.clone(),
                });
            }

            let mut field_path = prefix.to_vec();
            field_path.push(name.clone());

            if let Some(reference) = &field.assumes {
                self.links.push(Link {
                    chain: chain.clone(),
                    path: field_path.clone(),
                    modality,
                    modality_span,
                    reference: Reference::Assume(reference.clone()),
                });
            } else if !field.extends.is_empty() {
                self.links.push(Link {
                    chain: chain.clone(),
                    path: field_path.clone(),
                    modality,
                    modality_span,
                    reference: Reference::Extend(field.extends.clone()),
                });
            }

            let declared = variants
                .iter()
                .map(|declared| {
                    let mut path = field_path.clone();
                    path.push(declared.name.clone());

                    self.variants
                        .insert(path.clone(), (chain.clone(), declared.name.clone()));

                    self.locations.insert(
                        vec![
                            chain.0.to_snake_case(),
                            chain.1.to_snake_case(),
                            chain.2.to_snake_case(),
                            declared.name.to_snake_case(),
                        ],
                        Location {
                            head: declared.span,
                            domain: None,
                        },
                    );

                    if let Some(requires) = &declared.requires {
                        for pattern in &requires.inner.patterns {
                            for entitled in &pattern.inner.entitlements {
                                // a broken correspondence resolves no
                                // targets — phase two reports it
                                let Ok(segments) = corresponding(entitled, &arrays) else {
                                    continue;
                                };

                                let prefix = segments
                                    .into_iter()
                                    .map(|(name, ..)| name)
                                    .collect::<Vec<_>>();

                                match &entitled.inner.set {
                                    Some(set) => {
                                        for member in set {
                                            let mut target = prefix.clone();
                                            target.push(member.inner.to_string());
                                            self.edges.push((path.clone(), target, requires.span));
                                        }
                                    }
                                    None => {
                                        self.edges.push((path.clone(), prefix, requires.span));
                                    }
                                }
                            }
                        }

                        self.pending.push(Pending {
                            target: Target::Variant(chain.clone(), declared.name.clone()),
                            space: requires.clone(),
                            arrays: arrays.clone(),
                        });
                    }

                    VariantDecl {
                        variant: declared.variant.clone(),
                        side: declared.side.as_ref().map(side_decl),
                    }
                })
                .collect();

            out.push(FieldDecl {
                field: composed,
                access: access.clone(),
                variants: declared,
            });
        }

        out
    }

    /// The variants a field or schema body declares, arrays expanded.
    pub(super) fn variants_of(
        &mut self,
        body: &Option<Spanned<Vec<Spanned<FieldItem<'src>>>>>,
    ) -> Vec<DeclaredVariant<'src>> {
        let mut out = Vec::new();

        let Some(body) = body else {
            return out;
        };

        for item in &body.inner {
            let FieldItem::Variant(variant) = &item.inner else {
                continue;
            };

            let variant = match variant.head.template.clone() {
                None => variant.clone(),
                Some(template_ref) => {
                    self.redundant_as(&variant.head);

                    let Some(template) = self.resolve_template(
                        &template_ref,
                        "variant",
                        |templates, name| templates.variants.get(name).copied(),
                        |templates| {
                            templates
                                .variants
                                .keys()
                                .map(|name| name.to_string())
                                .collect()
                        },
                    ) else {
                        continue;
                    };

                    merge::variant(template.clone(), variant)
                }
            };

            if let Some(leaky) = variant.leaky {
                self.diagnostics
                    .push(Diagnostic::unsupported(leaky, "variant leakiness"));
            }

            let Some(name) = &variant.head.name else {
                self.diagnostics
                    .push(Diagnostic::unnamed(item.span, "variant"));
                continue;
            };

            let compose = |name: &str, bits: u32| {
                let mut composed = ::model::Variant::new(name, bits)
                    .docs(variant.docs.iter().map(|doc| doc.inner));

                if variant.inert.is_some() {
                    composed = composed.inert();
                }

                composed
            };

            match variant
                .array
                .or(variant.head.indices.as_ref().map(|indices| indices.span))
            {
                Some(..) => {
                    let Some(indices) = &variant.head.indices else {
                        self.diagnostics
                            .push(Diagnostic::expected_elements(item.span));
                        continue;
                    };

                    let Some(names) =
                        expand::element_names(name.inner, indices, 0, &mut self.diagnostics)
                    else {
                        continue;
                    };

                    let values = match &variant.value {
                        Some(value) => match &value.inner {
                            VariantValue::List(entries) => {
                                let entries = entries
                                    .iter()
                                    .map(|entry| Spanned {
                                        inner: match entry.inner {
                                            ValueEntry::Value(value) => ListEntry::Value(value),
                                            ValueEntry::Rest(rest) => ListEntry::Rest(rest),
                                        },
                                        span: entry.span,
                                    })
                                    .collect::<Vec<_>>();

                                expand::scalar_series(
                                    &entries,
                                    names.len(),
                                    Ok(1),
                                    value.span,
                                    &mut self.diagnostics,
                                )
                            }
                            VariantValue::Value(..) => {
                                self.diagnostics
                                    .push(Diagnostic::expected_value_list(value.span));
                                None
                            }
                        },
                        None => {
                            self.diagnostics.push(Diagnostic::expected_value(item.span));
                            None
                        }
                    };

                    let Some(values) = values else {
                        continue;
                    };

                    let head = head_span(
                        variant.head.name.as_ref(),
                        item.span,
                        &[
                            variant.head.indices.as_ref().map(|indices| indices.span),
                            variant.value.as_ref().map(|value| value.span),
                            variant.requires.as_ref().map(|requires| requires.span),
                        ],
                    );

                    out.extend(
                        names
                            .iter()
                            .zip(values)
                            .map(|(name, value)| DeclaredVariant {
                                name: name.clone(),
                                variant: compose(name, value),
                                span: head,
                                side: variant.side,
                                requires: variant.requires.clone(),
                            }),
                    );
                }
                None => {
                    let value = match &variant.value {
                        Some(value) => match &value.inner {
                            VariantValue::Value(value) => Some(*value),
                            VariantValue::List(..) => {
                                self.diagnostics.push(Diagnostic::scalar_value(value.span));
                                None
                            }
                        },
                        None => {
                            self.diagnostics.push(Diagnostic::expected_value(item.span));
                            None
                        }
                    };

                    let Some(value) = value else {
                        continue;
                    };

                    out.push(DeclaredVariant {
                        name: name.inner.to_string(),
                        variant: compose(name.inner, value),
                        span: head_span(
                            variant.head.name.as_ref(),
                            item.span,
                            &[
                                variant.value.as_ref().map(|value| value.span),
                                variant.requires.as_ref().map(|requires| requires.span),
                            ],
                        ),
                        side: variant.side,
                        requires: variant.requires.clone(),
                    });
                }
            }
        }

        out
    }

    /// Resolve a schema definition through its template (one level, as
    /// invocations do) and collect its declared variants.
    ///
    /// A schema has no access modality — any mix of plain, `read`, and
    /// `write` variants is a fine schema; sides are judged against each
    /// *referencing* field. Statewise requirements inside schema bodies are
    /// rejected here: entitlements are intrinsically application-time — they
    /// cannot be templated.
    pub(super) fn schema_shape(
        &mut self,
        schema: &Schema<'src>,
        _span: Span,
    ) -> Option<(Vec<DeclaredVariant<'src>>, Schema<'src>)> {
        let schema = match schema.head.template.clone() {
            None => schema.clone(),
            Some(template_ref) => {
                let template = self.resolve_template(
                    &template_ref,
                    "schema",
                    |templates, name| templates.schemas.get(name).copied(),
                    |templates| {
                        templates
                            .schemas
                            .keys()
                            .map(|name| name.to_string())
                            .collect()
                    },
                )?;

                merge::schema(template.clone(), schema)
            }
        };

        let name = schema
            .head
            .name
            .as_ref()
            .map(|name| name.inner)
            .unwrap_or("_")
            .to_string();

        let variants = self.variants_of(&schema.body);

        for declared in &variants {
            if let Some(requires) = &declared.requires {
                self.diagnostics.push(Diagnostic::templated_entitlements(
                    requires.span,
                    &declared.name,
                    &name,
                ));
            }
        }

        Some((variants, schema))
    }

    /// Place a schema: build its declaration (resolving its template), record
    /// it for scoped reference resolution, and queue its insertion.
    pub(super) fn place(
        &mut self,
        schema: &Schema<'src>,
        span: Span,
        scope: VariantPath,
        locator: Locator,
    ) {
        let Some((variants, schema)) = self.schema_shape(schema, span) else {
            return;
        };

        let Some(name) = &schema.head.name else {
            self.diagnostics.push(Diagnostic::unnamed(span, "schema"));
            return;
        };

        let key = (scope.clone(), name.inner.to_string());

        if let Some(existing) = self.schemas.get(&key) {
            self.diagnostics.push(Diagnostic::duplicate_placement(
                span,
                name.inner,
                existing.span,
            ));
            return;
        }

        self.locations.insert(
            vec![name.inner.to_string()],
            Location {
                head: name.span,
                domain: None,
            },
        );

        self.schemas.insert(
            key,
            PlacedSchema {
                variants: variants
                    .iter()
                    .map(|declared| {
                        (
                            declared.name.clone(),
                            declared
                                .side
                                .as_ref()
                                .map(|side| (side_decl(side), side.span)),
                            declared.span,
                        )
                    })
                    .collect(),
                index: None,
                span: name.span,
            },
        );

        self.placements.push(Placement {
            scope,
            locator,
            decl: SchemaDecl {
                ident: name.inner.to_string(),
                variants: variants
                    .into_iter()
                    .map(|declared| VariantDecl {
                        variant: declared.variant,
                        side: declared.side.as_ref().map(side_decl),
                    })
                    .collect(),
                docs: schema
                    .docs
                    .iter()
                    .map(|doc| doc.inner.to_string())
                    .collect(),
            },
        });
    }

    /// Insert every queued placement — the structures they sit within exist
    /// by now — recording the resulting indices for reference resolution.
    pub(super) fn place_schemas(&mut self, composition: &mut Composition) {
        let placements = std::mem::take(&mut self.placements);

        for Placement {
            scope,
            locator,
            decl,
        } in placements
        {
            let parent = match &locator {
                Locator::Root => None,
                Locator::PeripheralGroup(name) => Some(ParentIndex::PeripheralGroup(
                    Ident::new(name, IdentSpan::call_site()).into(),
                )),
                Locator::Peripheral(name) => Some(ParentIndex::Peripheral(ident(name).into())),
                Locator::RegisterGroup(name) => Some(ParentIndex::RegisterGroup(
                    Ident::new(name, IdentSpan::call_site()).into(),
                )),
                Locator::FieldGroup(name) => Some(ParentIndex::FieldGroup(
                    Ident::new(name, IdentSpan::call_site()).into(),
                )),
                Locator::Register(peripheral, register) => {
                    let Some(register) = composition
                        .try_get_peripheral(ident(peripheral).into())
                        .and_then(|peripheral| {
                            peripheral
                                .try_get_register(&ident(register))
                                .map(|register| *register.index())
                        })
                    else {
                        continue;
                    };

                    Some(ParentIndex::Register(register))
                }
            };

            let name = decl.ident.clone();
            let index = composition.insert_schema(decl, parent);

            if let Some(placed) = self.schemas.get_mut(&(scope, name)) {
                placed.index = Some(index);
            }
        }
    }

    pub(super) fn interrupts(
        &mut self,
        composition: &mut Composition,
        interrupts: &Interrupts<'src>,
    ) {
        // hover and inlay hints show each entry's vector position
        let base = self.vectors.len();

        for (offset, entry) in interrupts.entries.iter().enumerate() {
            let span = match &entry.inner.kind {
                InterruptKind::Reserved => entry.span,
                InterruptKind::Handler(name) => name.span,
            };

            self.vectors.push((span, base + offset));
        }

        composition.add_interrupts(interrupts.entries.iter().map(|entry| {
            match &entry.inner.kind {
                InterruptKind::Reserved => ::model::Interrupt::reserved(),
                InterruptKind::Handler(name) => ::model::Interrupt::handler(name.inner),
            }
            .docs(entry.inner.docs.iter().map(|doc| doc.inner))
        }));
    }
}
