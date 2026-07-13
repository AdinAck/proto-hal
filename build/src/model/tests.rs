//! Model evaluation, tested end to end: `.phm` source in, model shape and
//! diagnostic [`Kind`]s out.
//!
//! Every diagnostic in the [catalog](super::semantic) has at least one test
//! that provokes it, and the language's semantics — projection, sides,
//! resets, entitlements, templates — are pinned by positive cases.

use std::collections::HashMap;

use super::{Diagnostic, Evaluation, Kind, evaluate, evaluate_sources};
use crate::model::source::{SourceFile, Sources};

/// The kinds of every semantic diagnostic an evaluation produced.
fn kinds(src: &str) -> Vec<Kind> {
    evaluate(src)
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            Diagnostic::Semantic(semantic) => Some(semantic.kind),
            Diagnostic::Syntax(..) => None,
        })
        .collect()
}

/// Evaluate, asserting the description is entirely clean.
fn clean(src: &str) -> Evaluation<'_> {
    let evaluation = evaluate(src);

    assert!(
        evaluation.diagnostics.is_empty(),
        "expected a clean evaluation, got: {:?}",
        evaluation
            .diagnostics
            .iter()
            .map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic) => semantic.message.clone(),
                Diagnostic::Syntax(error) => format!("{error:?}"),
            })
            .collect::<Vec<_>>(),
    );

    evaluation
}

/// A `Sources` of an entry file plus imports, without touching a filesystem.
fn sources(entry: &str, imports: &[(&str, &str)]) -> Sources {
    let mut entry_imports = HashMap::new();

    for (id, (alias, ..)) in imports.iter().enumerate() {
        entry_imports.insert(alias.to_string(), id + 1);
    }

    Sources {
        files: std::iter::once(SourceFile {
            name: "entry.phm".to_string(),
            path: None,
            content: entry.to_string(),
            imports: entry_imports,
        })
        .chain(imports.iter().map(|(alias, content)| SourceFile {
            name: format!("{alias}.phm"),
            path: None,
            content: content.to_string(),
            imports: HashMap::new(),
        }))
        .collect(),
        issues: Vec::new(),
    }
}

mod structure {
    use super::*;

    #[test]
    fn minimal_device() {
        let evaluation = clean("device d { peripheral p @ 0x0 }");
        let model = evaluation.model.unwrap();

        assert_eq!(model.peripheral_count(), 1);
    }

    #[test]
    fn no_device() {
        assert!(kinds("schema s { variant A ~ 0 }").contains(&Kind::NoDevice));
    }

    #[test]
    fn many_devices() {
        assert!(kinds("device a { } device b { }").contains(&Kind::ManyDevices));
    }

    #[test]
    fn empty_device_warns() {
        assert!(kinds("device d").contains(&Kind::EmptyDevice));
    }

    #[test]
    fn unnamed_group() {
        assert!(
            kinds("device d { peripheral group { peripheral p @ 0x0 } }")
                .contains(&Kind::Unnamed)
        );
    }

    #[test]
    fn missing_position() {
        assert!(kinds("device d { peripheral p }").contains(&Kind::ExpectedPosition));
    }

    #[test]
    fn missing_modality() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register r @ 0x0 { field f @ 0 } } }")
                .contains(&Kind::ExpectedModality)
        );
    }

    #[test]
    fn field_exceeds_register() {
        // an offset past the register entirely is judged at elaboration
        assert!(
            kinds("device d { peripheral p @ 0x0 { register r @ 0x0 { read field f @ 35 } } }")
                .contains(&Kind::DoesNotFit)
        );

        // a domain leaking over the edge is judged by the model
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 {
                    read field f @ 30..=33
                } } }"
            )
            .contains(&Kind::ExceedsDomain)
        );
    }

    #[test]
    fn groups_at_every_level() {
        let evaluation = clean(
            "device d {
                peripheral group pg {
                    peripheral p @ 0x0 {
                        register group rg {
                            register r @ 0x0 reset 0 {
                                field group fg {
                                    store field f @ 0 { variant A ~ 0 }
                                }
                            }
                        }
                    }
                }
            }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.field_count(), 1);
    }
}

mod arrays {
    use super::*;

    /// `[0..=3, ...]` designators continue across the enclosing array's
    /// elements.
    #[test]
    fn designators_continue_across_parents() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 {
                register array r[0..=1] @ [0x0, ...] {
                    read field array f[0..=3, ...] @ [0, ...]
                }
            } }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.field_count(), 8);

        let generated = model.render_raw();
        assert!(generated.contains("pub mod f0"), "r0 starts the series");
        assert!(generated.contains("pub mod f7"), "r1 continues it");
    }

    #[test]
    fn expansion_with_strides() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 {
                register array r[0..=3] @ [0x0, ...] {
                    store field array f[a, b] @ [0..=1, ...] reset 0 {
                        variant array V[0..=2] ~ [0, ...]
                    }
                }
            } }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.register_count(), 4);
        assert_eq!(model.field_count(), 8);
        assert_eq!(model.variant_count(), 24);
    }

    #[test]
    fn peripherals_may_not_auto_increment() {
        assert!(
            kinds("device d { peripheral array p[0..=1] @ [0x0, ...] }")
                .contains(&Kind::UnsupportedRest)
        );
    }

    #[test]
    fn array_without_designators() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register array r @ [0x0, 0x4] } }")
                .contains(&Kind::ExpectedElements)
        );
    }

    #[test]
    fn designators_without_array() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register r[0..=1] @ [0x0, 0x4] } }")
                .contains(&Kind::ExpectedArray)
        );
    }

    #[test]
    fn array_without_position_list() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register array r[0..=1] @ 0x0 } }")
                .contains(&Kind::ExpectedPositions)
        );
    }

    #[test]
    fn scalar_position_takes_no_bit_domain() {
        assert!(
            kinds("device d { peripheral p @ 0..=1 }").contains(&Kind::ExpectedScalar)
        );
    }

    #[test]
    fn variant_without_value() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field f @ 0 { variant A }
                } } }"
            )
            .contains(&Kind::ExpectedValue)
        );
    }

    #[test]
    fn empty_range() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register array r[3..=0] @ [0x0, ...] } }")
                .contains(&Kind::EmptyRange)
        );
    }

    #[test]
    fn count_mismatch() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register array r[0..=2] @ [0x0, 0x4] } }")
                .contains(&Kind::CountMismatch)
        );
    }

    #[test]
    fn dangling_rest() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register array r[0..=2] @ [...] } }")
                .contains(&Kind::DanglingRest)
        );
    }

    #[test]
    fn negative_stride_below_zero() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 {
                    register array r[0..=2] @ [0x4, ...-0x4]
                } }"
            )
            .contains(&Kind::OutOfRange)
        );
    }
}

mod resets {
    use super::*;

    #[test]
    fn field_level_reset() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 {
                store field f @ 0..=1 reset 0x2 { variant A ~ 0 variant B ~ 2 }
            } } }",
        );
    }

    #[test]
    fn reset_by_variant_name() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 {
                store field f @ 0..=1 reset B { variant A ~ 0 variant B ~ 2 }
            } } }",
        );
    }

    #[test]
    fn reset_by_unknown_name() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 {
                    store field f @ 0..=1 reset Nope { variant A ~ 0 }
                } } }"
            )
            .contains(&Kind::UnknownVariant)
        );
    }

    #[test]
    fn redundant_agreement_is_permitted() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0x2 {
                store field f @ 0..=1 reset 0x2 { variant A ~ 0 variant B ~ 2 }
            } } }",
        );
    }

    #[test]
    fn contradiction_never_is() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0x1 {
                    store field f @ 0..=1 reset 0x2 { variant A ~ 1 variant B ~ 2 }
                } } }"
            )
            .contains(&Kind::ContradictoryReset)
        );
    }

    #[test]
    fn resolvable_field_needs_a_reset() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 {
                    store field f @ 0 { variant A ~ 0 }
                } } }"
            )
            .contains(&Kind::ExpectedReset)
        );
    }

    #[test]
    fn reset_must_name_a_variant_value() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0x3 {
                    store field f @ 0..=1 { variant A ~ 0 variant B ~ 1 }
                } } }"
            )
            .contains(&Kind::InvalidReset)
        );
    }

    #[test]
    fn register_resets_are_numeric() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register r @ 0x0 reset On } }")
                .contains(&Kind::NonNumericRegisterReset)
        );
    }
}

mod schemas {
    use super::*;

    /// One vocabulary, projected onto three modalities.
    #[test]
    fn vocabulary_projection() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                schema enable { variant Off ~ 0 variant On ~ 1 }

                store field a @ 0..=1 assumes enable
                read write field b @ 2..=3 assumes enable
                read field c @ 4..=5 assumes enable
            } } }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.schema_count(), 1);
        assert_eq!(model.field_count(), 3);
    }

    /// A read variant lands only on the read side of a `read write` field.
    #[test]
    fn sides_project() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 {
                schema status {
                    read variant Busy ~ 1
                    variant Idle ~ 0
                }

                read write field f @ 0..=1 assumes status
            } } }",
        );

        let model = evaluation.model.unwrap();
        let field = model
            .peripherals()
            .next()
            .unwrap()
            .registers()
            .next()
            .unwrap()
            .fields()
            .next()
            .unwrap();

        let reads = field.access.access().get_read().unwrap();
        let writes = field.access.access().get_write().unwrap();

        let count = |numericity: &::model::field::numericity::Numericity| match numericity {
            ::model::field::numericity::Numericity::Enumerated(enumerated) => {
                enumerated.variants.len()
            }
            ::model::field::numericity::Numericity::Numeric(..) => 0,
        };

        assert_eq!(count(reads), 2, "read side: Busy and Idle");
        assert_eq!(count(writes), 1, "write side: Idle only");
    }

    #[test]
    fn extends_accumulate() {
        let evaluation = clean(
            "schema flags { read variant Ok ~ 0 }
            schema commands { write variant Halt ~ 1 }

            device d { peripheral p @ 0x0 { register r @ 0x0 {
                read write field f @ 0..=1 extends flags, commands {
                    variant Extra ~ 2
                }
            } } }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.variant_count(), 3);
    }

    #[test]
    fn template_extends_accumulate_with_invocation() {
        clean(
            "schema flags { variant Ok ~ 0 }
            schema more { variant Extra ~ 1 }

            store field base @ 0..=1 extends flags

            device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                field #base as f @ 0..=1 extends more
            } } }",
        );
    }

    #[test]
    fn read_variant_cannot_occupy_write_field() {
        assert!(
            kinds(
                "schema status { read variant Busy ~ 1 }
                device d { peripheral p @ 0x0 { register r @ 0x0 {
                    write field f @ 0 extends status
                } } }"
            )
            .contains(&Kind::ContradictorySide)
        );
    }

    #[test]
    fn sided_variant_cannot_occupy_store_field() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field f @ 0 { read variant A ~ 0 }
                } } }"
            )
            .contains(&Kind::InvalidSide)
        );
    }

    #[test]
    fn redundant_side_is_permitted() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 {
                read field f @ 0 { read variant A ~ 0 }
            } } }",
        );
    }

    /// The compatibility error names both parties, with a label each.
    #[test]
    fn incompatibility_labels_both_parties() {
        let evaluation = evaluate(
            "schema status { read variant Busy ~ 1 }
            device d { peripheral p @ 0x0 { register r @ 0x0 {
                write field f @ 0 extends status
            } } }",
        );

        let semantic = evaluation
            .diagnostics
            .iter()
            .find_map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic)
                    if semantic.kind == Kind::ContradictorySide =>
                {
                    Some(semantic)
                }
                _ => None,
            })
            .expect("a side contradiction");

        assert_eq!(semantic.labels.len(), 2, "the side token and the modality");
    }

    #[test]
    fn assuming_requires_placement() {
        assert!(
            kinds(
                "schema enable { variant On ~ 1 }
                device d { peripheral p @ 0x0 { register r @ 0x0 {
                    store field f @ 0 assumes enable reset On
                } } }"
            )
            .contains(&Kind::UnplacedSchema)
        );
    }

    #[test]
    fn extension_requires_no_placement() {
        clean(
            "schema enable { variant Off ~ 0 variant On ~ 1 }
            device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                store field f @ 0 extends enable
            } } }",
        );
    }

    #[test]
    fn nearest_placement_wins() {
        clean(
            "device d {
                schema s { variant Outer ~ 0 }

                peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    schema s { variant Inner ~ 0 }

                    store field f @ 0 assumes s reset Inner
                } }
            }",
        );
    }

    #[test]
    fn assuming_field_declares_no_variants() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 {
                    schema s { variant A ~ 0 }
                    store field f @ 0 assumes s reset A { variant B ~ 1 }
                } } }"
            )
            .contains(&Kind::AssumedVariants)
        );
    }

    #[test]
    fn assumes_and_extends_are_exclusive() {
        assert!(
            kinds(
                "schema a { variant A ~ 0 }
                schema b { variant B ~ 1 }
                device d { peripheral p @ 0x0 { register r @ 0x0 {
                    store field f @ 0 assumes a extends b
                } } }"
            )
            .contains(&Kind::AssumesAndExtends)
        );
    }

    #[test]
    fn schema_variants_cannot_have_requirements() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    schema s { variant A ~ 0 requires p.r.f.B }
                    store field f @ 0 { variant B ~ 0 }
                } } }"
            )
            .contains(&Kind::TemplatedEntitlements)
        );
    }

    #[test]
    fn placements_are_unique_per_scope() {
        assert!(
            kinds(
                "device d {
                    schema s { variant A ~ 0 }
                    schema s { variant B ~ 1 }
                    peripheral p @ 0x0
                }"
            )
            .contains(&Kind::Exists)
        );
    }
}

mod entitlements {
    use super::*;

    /// `field.{A, B}` — one pattern, the field among the set.
    #[test]
    fn variant_sets_form_one_pattern() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                store field scale @ 0..=1 { variant array N[0..=3] ~ [0, ...] }
                store field func @ 2..=3 {
                    variant Plain ~ 0
                    variant Sqrt ~ 1
                        requires p.r.scale.{N0, N1, N2}
                }
            } } }",
        );
    }

    /// Set members resolve individually: a misspelled member narrows to
    /// itself.
    #[test]
    fn set_members_narrow() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field scale @ 0..=1 { variant array N[0..=2] ~ [0, ...] }
                    store field func @ 2..=3 {
                        variant Plain ~ 0
                        variant Sqrt ~ 1
                            requires p.r.scale.{N0, N9}
                    }
                } } }"
            )
            .contains(&Kind::UnknownPath)
        );
    }

    /// `&` conjoins distinct fields — naming one twice is a contradiction.
    #[test]
    fn repeated_fields_are_rejected() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field scale @ 0..=1 { variant array N[0..=2] ~ [0, ...] }
                    store field func @ 2..=3 {
                        variant Plain ~ 0
                        variant Sqrt ~ 1
                            requires p.r.scale.N0 & p.r.scale.N1
                    }
                } } }"
            )
            .contains(&Kind::RepeatedField)
        );
    }

    #[test]
    fn requirements_resolve_forwards_and_backwards() {
        clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                store field a @ 0
                    requires p.r.b.On
                { variant A ~ 0 }

                store field b @ 1 { variant Off ~ 0 variant On ~ 1 }
            } } }",
        );
    }

    #[test]
    fn read_only_fields_admit_no_write_entitlements() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    read field f @ 0 write requires p.r.g.On
                    store field g @ 1 { variant Off ~ 0 variant On ~ 1 }
                } } }"
            )
            .contains(&Kind::EntitlementModality)
        );
    }

    #[test]
    fn hardware_writes_need_volatile_store() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field f @ 0
                        hardware write requires p.r.g.On
                    { variant A ~ 0 }
                    store field g @ 1 { variant Off ~ 0 variant On ~ 1 }
                } } }"
            )
            .contains(&Kind::EntitlementModality)
        );
    }

    /// An unresolved path is narrowed to the failing segment.
    #[test]
    fn unknown_paths_narrow_to_the_failing_segment() {
        let evaluation = evaluate(
            "device d { peripheral cordic @ 0x0 { register csr @ 0x0 reset 0 {
                store field func @ 0..=1 { variant Cos ~ 0 }
                store field scale @ 2
                    requires cordic.csr.fnuc.Cos
                { variant N0 ~ 0 }
            } } }",
        );

        let semantic = evaluation
            .diagnostics
            .iter()
            .find_map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic) if semantic.kind == Kind::UnknownPath => {
                    Some(semantic)
                }
                _ => None,
            })
            .expect("an unknown path");

        assert_eq!(
            semantic.span.end - semantic.span.start,
            "fnuc".len(),
            "the span covers exactly the failing segment",
        );
        assert!(
            semantic
                .notes
                .iter()
                .any(|note| note.contains("similarly named")),
            "the near-miss is suggested",
        );
    }

    #[test]
    fn incomplete_paths_are_reported() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field a @ 0 requires p.r.b { variant A ~ 0 }
                    store field b @ 1 { variant On ~ 1 }
                } } }"
            )
            .contains(&Kind::UnknownPath)
        );
    }

    #[test]
    fn statewise_cycles_are_detected() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field a @ 0 { variant On ~ 1 requires p.r.b.On variant Off ~ 0 }
                    store field b @ 1 { variant On ~ 1 requires p.r.a.On variant Off ~ 0 }
                } } }"
            )
            .contains(&Kind::EntitlementCycle)
        );
    }
}

mod style {
    use super::*;

    /// `#template as template` — the name it would take anyway.
    #[test]
    fn redundant_as_warns() {
        assert!(
            kinds(
                "peripheral port { register r @ 0x0 }
                device d { peripheral #port as port @ 0x0 }"
            )
            .contains(&Kind::RedundantAs)
        );

        // a renaming `as` is quiet
        assert!(
            !kinds(
                "peripheral port { register r @ 0x0 }
                device d { peripheral #port as gpioa @ 0x0 }"
            )
            .contains(&Kind::RedundantAs)
        );
    }
}

mod correspondence {
    use super::*;

    /// `ccr[0..2]` — element `i` of the writer requires element `i` of the
    /// targets.
    #[test]
    fn correspondence_zips_by_ordinal() {
        let evaluation = clean(
            "device d { peripheral dma @ 0x0 {
                register array ccr[0..2] @ [0x8, ...+0x14] reset 0 {
                    store field en @ 0 { variant Disabled ~ 0 variant Enabled ~ 1 }
                }
                register array cpar[0..2] @ [0x10, ...+0x14] reset 0 {
                    store field pa @ 0..=31
                        requires dma.ccr[0..2].en.Disabled
                }
            } }",
        );

        let generated = evaluation.model.unwrap().render_raw();

        let cpar0 = generated.find("pub mod cpar0").expect("cpar0 renders");
        let cpar1 = generated.find("pub mod cpar1").expect("cpar1 renders");
        let to_ccr0 = generated.find("ccr0 :: en :: En").expect("an entitlement to ccr0");
        let to_ccr1 = generated.find("ccr1 :: en :: En").expect("an entitlement to ccr1");

        assert!(
            cpar0 < to_ccr0 && to_ccr0 < cpar1,
            "cpar0's pa requires ccr0's en",
        );
        assert!(cpar1 < to_ccr1, "cpar1's pa requires ccr1's en");
        assert_eq!(
            generated.matches(":: en :: En <").count(),
            2,
            "one entitlement per element, no cross-wiring",
        );
    }

    /// Correspondence is by name — the bracket may start at any designator
    /// the targets carry.
    #[test]
    fn correspondence_is_by_name() {
        clean(
            "device d { peripheral dma @ 0x0 {
                register array ccr[0..3] @ [0x8, ...+0x14] reset 0 {
                    store field en @ 0 { variant Disabled ~ 0 variant Enabled ~ 1 }
                }
                register array cpar[0..2] @ [0x20, ...+0x14] reset 0 {
                    store field pa @ 0..=31
                        requires dma.ccr[1..3].en.Disabled
                }
            } }",
        );
    }

    /// Statewise requirements correspond too.
    #[test]
    fn variant_requirements_correspond() {
        clean(
            "device d { peripheral dma @ 0x0 {
                register array ccr[0..2] @ [0x8, ...+0x14] reset 0 {
                    store field en @ 0 { variant Disabled ~ 0 variant Enabled ~ 1 }
                }
                register array cmar[0..2] @ [0x10, ...+0x14] reset 0 {
                    store field ma @ 0 {
                        variant Idle ~ 0
                        variant Armed ~ 1
                            requires dma.ccr[0..2].en.Enabled
                    }
                }
            } }",
        );
    }

    #[test]
    fn correspondence_outside_any_array_is_rejected() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field f @ 0
                        requires p.g[0..2].x.On
                    { variant A ~ 0 }
                } } }"
            )
            .contains(&Kind::InvalidCorrespondence)
        );
    }

    /// One bracket binds one enclosing array — nested arrays are v2.
    #[test]
    fn nested_correspondence_is_not_yet_supported() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 {
                    register array r[0..2] @ [0x0, ...] {
                        read field array f[0..2, ...] @ [0, ...]
                            requires p.s[0..2].x.On
                    }
                } }"
            )
            .contains(&Kind::InvalidCorrespondence)
        );
    }

    #[test]
    fn several_correspondences_are_not_yet_supported() {
        assert!(
            kinds(
                "device d { peripheral dma @ 0x0 {
                    register array ccr[0..2] @ [0x8, ...+0x14] reset 0 {
                        store field en @ 0 { variant Disabled ~ 0 variant Enabled ~ 1 }
                    }
                    register array cpar[0..2] @ [0x10, ...+0x14] reset 0 {
                        store field pa @ 0..=31
                            requires dma.ccr[0..2].en[0..2].Disabled
                    }
                } }"
            )
            .contains(&Kind::InvalidCorrespondence)
        );
    }

    /// The bracket must name exactly one target per element — and the
    /// failure is reported once, not once per element.
    #[test]
    fn correspondence_length_must_match() {
        let report = kinds(
            "device d { peripheral dma @ 0x0 {
                register array ccr[0..3] @ [0x8, ...+0x14] reset 0 {
                    store field en @ 0 { variant Disabled ~ 0 variant Enabled ~ 1 }
                }
                register array cpar[0..2] @ [0x20, ...+0x14] reset 0 {
                    store field pa @ 0..=31
                        requires dma.ccr[0..3].en.Disabled
                }
            } }",
        );

        assert_eq!(
            report
                .iter()
                .filter(|kind| **kind == Kind::InvalidCorrespondence)
                .count(),
            1,
        );
    }
}

mod templates {
    use super::*;

    #[test]
    fn invocations_override_and_append() {
        let evaluation = clean(
            "register base @ 0x0 reset 0 {
                store field f @ 0 { variant A ~ 0 }
            }

            device d { peripheral p @ 0x0 {
                register #base as r @ 0x4 {
                    store field g @ 1 { variant B ~ 0 }
                }
            } }",
        );

        let model = evaluation.model.unwrap();
        assert_eq!(model.field_count(), 2, "template's field plus the appended one");
    }

    #[test]
    fn unknown_template() {
        let evaluation = evaluate("device d { peripheral #nope as p @ 0x0 }");

        let semantic = evaluation
            .diagnostics
            .iter()
            .find_map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic) if semantic.kind == Kind::UnknownTemplate => {
                    Some(semantic)
                }
                _ => None,
            })
            .expect("an unknown template");

        assert!(semantic.message.contains("`nope`"));
    }

    #[test]
    fn misspelled_template_is_suggested() {
        let evaluation = evaluate(
            "peripheral gpio_port @ 0x0
            device d { peripheral #gpio_prot as p @ 0x0 }",
        );

        assert!(
            evaluation.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                Diagnostic::Semantic(semantic)
                    if semantic.notes.iter().any(|note| note.contains("similarly named"))
            )),
        );
    }

    #[test]
    fn unknown_import() {
        assert!(
            kinds("device d { peripheral #st.gpio as p @ 0x0 }").contains(&Kind::UnknownImport)
        );
    }

    #[test]
    fn reference_depth_is_bounded() {
        assert!(
            kinds(
                "peripheral #b as a
                peripheral #a as b
                device d { peripheral #a as p @ 0x0 }"
            )
            .contains(&Kind::TemplateDepthExceeded)
        );
    }

    #[test]
    fn references_name_at_most_two_segments() {
        assert!(
            kinds("device d { peripheral #a.b.c as p @ 0x0 }")
                .contains(&Kind::InvalidTemplateReference)
        );
    }
}

mod imports {
    use super::*;

    #[test]
    fn definitions_reach_through_aliases() {
        let sources = sources(
            "import gpio
            device d { peripheral #gpio.port as p @ 0x0 }",
            &[("gpio", "peripheral port { register r @ 0x0 }")],
        );

        let evaluation = evaluate_sources(&sources);
        assert!(!evaluation.failed());
        assert_eq!(evaluation.model.unwrap().register_count(), 1);
    }

    #[test]
    fn imported_devices_are_templates() {
        let sources = sources(
            "import base
            device #base.core as d {
                peripheral extra @ 0x100
            }",
            &[("base", "device core { peripheral p @ 0x0 }")],
        );

        let evaluation = evaluate_sources(&sources);
        let model = evaluation.model.unwrap();

        assert_eq!(model.peripheral_count(), 2, "template's peripheral plus the appended one");
        assert!(
            !evaluation.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                Diagnostic::Semantic(semantic)
                    if semantic.kind == Kind::DeviceNotElaborated
            )),
            "a device that served as a template earns no warning",
        );
    }

    #[test]
    fn unused_imported_devices_warn() {
        let sources = sources(
            "import base
            device d { peripheral p @ 0x0 }",
            &[("base", "device core { peripheral q @ 0x0 }")],
        );

        let evaluation = evaluate_sources(&sources);

        assert!(
            evaluation.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                Diagnostic::Semantic(semantic)
                    if semantic.kind == Kind::DeviceNotElaborated
            )),
            "an imported device nothing references still warns",
        );
    }

    /// Within a `phm.toml`-rooted model, the device's imports resolve
    /// within `components/`.
    #[test]
    fn components_satisfy_device_imports() {
        let model = std::env::temp_dir().join(format!("phm-components-{}", std::process::id()));
        let components = model.join("components/peripherals");

        std::fs::create_dir_all(&components).unwrap();
        std::fs::create_dir_all(model.join("devices")).unwrap();
        std::fs::write(model.join("phm.toml"), "").unwrap();
        std::fs::write(
            model.join("devices/d.phm"),
            "import peripherals.lib

            device d { peripheral #lib.port as p @ 0x0 }",
        )
        .unwrap();
        std::fs::write(
            components.join("lib.phm"),
            "peripheral port { register r @ 0x0 }",
        )
        .unwrap();

        let sources = crate::model::load(model.join("devices/d.phm")).unwrap();
        let evaluation = evaluate_sources(&sources);
        let messages = evaluation
            .diagnostics
            .iter()
            .map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic) => semantic.message.clone(),
                Diagnostic::Syntax(error) => format!("{error:?}"),
            })
            .collect::<Vec<_>>();

        std::fs::remove_dir_all(&model).unwrap();

        assert_eq!(messages, Vec::<String>::new());
        assert_eq!(sources.files.len(), 2, "the component was loaded");
    }
}

mod provided {
    use super::*;
    use crate::model::load_with;

    /// A dependency's descriptions satisfy imports before the filesystem —
    /// including the pack's own internal imports.
    #[test]
    fn provided_sources_satisfy_imports() {
        let dir = std::env::temp_dir().join("phm-provided-test");
        std::fs::create_dir_all(&dir).unwrap();

        let entry = dir.join("entry.phm");
        std::fs::write(
            &entry,
            "import cortex_m.nvic
            device d { peripheral #nvic.nvic_m0 as nvic @ 0xe000_e000 }",
        )
        .unwrap();

        let sources = load_with(
            &entry,
            &[
                (
                    "cortex_m/nvic.phm",
                    "import common
                    peripheral nvic_m0 { register #common.stub as iser1 @ 0x100 }",
                ),
                ("cortex_m/common.phm", "register stub @ 0x0"),
            ],
        )
        .unwrap();

        let evaluation = evaluate_sources(&sources);

        assert!(
            !evaluation.failed(),
            "provided imports resolve: {:?}",
            evaluation
                .diagnostics
                .iter()
                .map(|diagnostic| match diagnostic {
                    Diagnostic::Semantic(semantic) => semantic.message.clone(),
                    Diagnostic::Syntax(error) => format!("{error:?}"),
                })
                .collect::<Vec<_>>(),
        );
        assert_eq!(evaluation.model.unwrap().register_count(), 1);
    }
}

mod unsupported {
    use super::*;

    #[test]
    fn variant_leakiness_warns() {
        assert!(
            kinds(
                "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                    store field f @ 0 { leaky variant A ~ 0 }
                } } }"
            )
            .contains(&Kind::Unsupported)
        );
    }
}

mod model_judgements {
    use super::*;

    /// Model-phase diagnostics arrive anchored, like any other.
    #[test]
    fn overlap_is_anchored_to_the_register() {
        let evaluation = evaluate(
            "device d { peripheral p @ 0x0 { register r @ 0x0 {
                read field a @ 0..=3
                read field b @ 2..=5
            } } }",
        );

        let semantic = evaluation
            .diagnostics
            .iter()
            .find_map(|diagnostic| match diagnostic {
                Diagnostic::Semantic(semantic) if semantic.kind == Kind::Overlap => {
                    Some(semantic)
                }
                _ => None,
            })
            .expect("an overlap");

        assert_ne!(semantic.span.end, 0, "the judgement carries a source span");
    }

    #[test]
    fn unaligned_registers_are_rejected() {
        assert!(
            kinds("device d { peripheral p @ 0x0 { register r @ 0x2 } }")
                .contains(&Kind::AddressUnaligned)
        );
    }
}

mod rendering {
    use crate::model::{evaluate_sources, report::rendered, source::Sources};

    /// Render every diagnostic of a source, colors stripped.
    fn plain(src: &str) -> String {
        let sources = Sources::single("test.phm", src);
        let evaluation = evaluate_sources(&sources);

        let mut out = String::new();

        for diagnostic in &evaluation.diagnostics {
            out.push_str(&rendered(&sources, diagnostic));
        }

        // strip ANSI color escapes
        let mut stripped = String::new();
        let mut chars = out.chars();

        while let Some(character) = chars.next() {
            if character == '\u{1b}' {
                for terminator in chars.by_ref() {
                    if terminator == 'm' {
                        break;
                    }
                }
            } else {
                stripped.push(character);
            }
        }

        stripped
    }

    /// However many labels a line carries, it renders once.
    #[test]
    fn labeled_lines_render_once() {
        let report = plain(
            "schema status { read variant Busy ~ 1 }
device t { peripheral p @ 0x0 { register r @ 0x0 {
    write field f @ 0 extends status
} } }",
        );

        let occurrences = report
            .lines()
            .filter(|line| line.contains("write field f @ 0 extends status"))
            .count();

        assert_eq!(occurrences, 1, "one line, every arrow beneath it:\n{report}");
    }

    /// Only labeled lines render — but nearby ones merge, the lines between
    /// them shown rather than elided.
    #[test]
    fn nearby_ranges_merge() {
        let report = plain(
            "device t { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
    store field h @ 3 reset 0 {
        // a line between the parties
        read variant Bad ~ 0
    }
} } }",
        );

        assert!(
            report.contains("a line between the parties"),
            "the separating line renders:\n{report}",
        );
        assert!(!report.contains("┆"), "no gap for a merge:\n{report}");
        assert!(
            !report.contains("register r"),
            "unlabeled neighbors stay out:\n{report}",
        );
    }

    /// Distant ranges are separated, not interpolated.
    #[test]
    fn distant_ranges_gap() {
        let report = plain(
            "schema status { read variant Busy ~ 1 }
//
//
//
//
//
device t { peripheral p @ 0x0 { register r @ 0x0 {
    write field f @ 0 extends status
} } }",
        );

        assert!(report.contains("┆"), "a gap between distant ranges:\n{report}");
    }

    /// Notes are lowercase and unnumbered.
    #[test]
    fn notes_are_lowercase() {
        let report = plain("device t { peripheral p @ 0x0 { register r @ 0x2 } }");

        assert!(report.contains("note:"), "{report}");
        assert!(!report.contains("Note"), "{report}");
    }

    /// Deep nesting doesn't push the window off to the right.
    #[test]
    fn common_indent_is_eliminated() {
        let report = plain(
            "device t {
    peripheral p @ 0x0 {
        register r @ 0x0 {
            read field g @ 0 {
                write variant Nope ~ 1
            }
        }
    }
}",
        );

        assert!(
            report.lines().any(|line| line.contains("│ read field g")),
            "the shared indent is gone entirely:\n{report}",
        );
    }

    /// The primary label describes the span, not the message.
    #[test]
    fn primary_labels_are_not_the_message() {
        let report = plain("device t { peripheral #nope as p @ 0x0 }");

        assert!(report.contains("no such template"), "{report}");
    }
}

mod analysis {
    use super::*;

    /// Every segment of a resolved entitlement path names its element, for
    /// hover and go-to-definition.
    #[test]
    fn mentions_name_every_level() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                store field a @ 0 requires p.r.b.On { variant A ~ 0 }
                store field b @ 1 { variant Off ~ 0 variant On ~ 1 }
            } } }",
        );

        let named = |path: &[&str]| {
            evaluation
                .analysis
                .mentions
                .iter()
                .any(|(.., target)| target == path)
        };

        assert!(named(&["p"]));
        assert!(named(&["p", "r"]));
        assert!(named(&["p", "r", "b"]));
        assert!(named(&["p", "r", "b", "on"]));

        // every mention's target is a known definition
        for (.., target) in &evaluation.analysis.mentions {
            assert!(
                evaluation.analysis.locations.contains_key(target),
                "unlocated mention target: {target:?}",
            );
        }
    }

    /// Template and schema references record their definition sites.
    #[test]
    fn references_record_definition_sites() {
        let evaluation = clean(
            "schema enable { variant Off ~ 0 variant On ~ 1 }
            register template_r @ 0x0 reset 0 {
                store field f @ 0 assumes enable
            }

            device d {
                schema #enable
                peripheral p @ 0x0 { register #template_r as r }
            }",
        );

        // `#enable`, `#template_r`, and `assumes enable` all resolved
        assert!(
            evaluation.analysis.definitions.len() >= 3,
            "sites: {:?}",
            evaluation.analysis.definitions,
        );

        // and every template's own name is its own site
        assert!(
            evaluation
                .analysis
                .definitions
                .iter()
                .any(|(reference, site)| reference == site),
            "template names hover themselves",
        );
    }

    /// Interrupt entries carry their vector positions — reserved slots
    /// included.
    #[test]
    fn interrupt_entries_carry_their_positions() {
        let evaluation = clean(
            "device d {
                peripheral p @ 0x0
                interrupts { a reserved b }
            }",
        );

        let positions = evaluation
            .analysis
            .vectors
            .iter()
            .map(|(.., position)| *position)
            .collect::<Vec<_>>();

        assert_eq!(positions, vec![0, 1, 2]);
    }

    /// The completion tree offers every valid continuation.
    #[test]
    fn tree_offers_continuations() {
        let evaluation = clean(
            "device d { peripheral p @ 0x0 { register r @ 0x0 reset 0 {
                store field a @ 0 { variant A ~ 0 }
                store field b @ 1 { variant Off ~ 0 variant On ~ 1 }
            } } }",
        );

        let children = evaluation
            .analysis
            .tree
            .get(&vec!["p".to_string(), "r".to_string()])
            .expect("the register prefix is a valid continuation point");

        assert!(children.contains(&"a".to_string()));
        assert!(children.contains(&"b".to_string()));
    }
}
