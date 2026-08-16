//! The grammar, tested item by item: source in, AST shape and error counts
//! out.
//!
//! Top-level items are noncorporeal definitions, so most tests parse a
//! single bare definition — no device wrapper needed.

use crate::{
    ast::{Access, File, FileItem, ResetValue, Side, VariantValue},
    parse,
};

/// Parse, asserting the source is entirely well-formed.
fn file(src: &str) -> File<'_> {
    let (file, errors) = parse(src, 0);

    assert!(errors.is_empty(), "unexpected syntax errors: {errors:#?}");

    file.expect("a file")
}

/// The number of syntax errors the source produces.
fn errors(src: &str) -> usize {
    parse(src, 0).1.len()
}

mod numbers {
    use super::*;

    #[test]
    fn every_radix() {
        let file = file(
            "register a @ 0x10
            register b @ 0b101
            register c @ 0o17
            register d @ 42",
        );

        let offsets = file
            .items
            .iter()
            .map(|item| match &item.inner {
                FileItem::Register(register) => match register.domain.as_ref().unwrap().inner {
                    crate::ast::Domain::Value(value) => value,
                    _ => panic!("a scalar offset"),
                },
                _ => panic!("a register"),
            })
            .collect::<Vec<_>>();

        assert_eq!(offsets, [0x10, 0b101, 0o17, 42]);
    }

    #[test]
    fn separators() {
        file("register r @ 0x4800_0000");
    }

    #[test]
    fn invalid_numbers_are_reported_not_fatal() {
        // the lexer never fails: the bad literal lexes as one unrecognized
        // token and the parser reports it in context
        let (file, errors) = parse("register r @ 0xzz", 0);

        assert!(file.is_some());
        assert_eq!(errors.len(), 1);
    }
}

mod fields {
    use super::*;

    fn field(src: &str) -> crate::ast::Field<'_> {
        match file(src).items.into_iter().next().unwrap().inner {
            FileItem::Field(field) => field,
            _ => panic!("a field"),
        }
    }

    #[test]
    fn access_modalities_compose_as_a_set() {
        assert_eq!(field("read write field f @ 0").access.len(), 2);
        assert_eq!(field("write read field f @ 0").access.len(), 2);
        assert_eq!(field("store field f @ 0").access.len(), 1);

        assert!(matches!(
            field("volatile store field f @ 0").access[0].inner,
            Access::VolatileStore,
        ));
    }

    #[test]
    fn head_properties_in_canonical_order() {
        let field = field(
            "store field f @ 0..=1
                extends enable
                reset Disabled
                requires p.r.g.On
                write requires p.r.h.On",
        );

        assert_eq!(field.extends.len(), 1);
        assert!(matches!(
            field.reset.unwrap().inner,
            ResetValue::Variant("Disabled"),
        ));
        assert!(field.requires.plain.is_some());
        assert!(field.requires.write.is_some());
    }

    #[test]
    fn assumes_takes_one_schema() {
        let field = field("store field f @ 0 assumes enable");

        assert!(field.assumes.is_some());
        assert!(field.extends.is_empty());
    }

    #[test]
    fn extends_comma_lists_and_repeated_clauses_accumulate() {
        assert_eq!(
            field("read write field f @ 0 extends a, b").extends.len(),
            2
        );
        assert_eq!(
            field("read write field f @ 0 extends a extends b, c")
                .extends
                .len(),
            3,
        );
    }

    #[test]
    fn numeric_resets_and_variant_resets() {
        assert!(matches!(
            field("store field f @ 0 reset 0x7f").reset.unwrap().inner,
            ResetValue::Value(0x7f),
        ));
    }

    #[test]
    fn hardware_write_requires() {
        assert!(
            field("volatile store field f @ 0 hardware write requires p.r.g.On")
                .requires
                .hardware_write
                .is_some()
        );
    }

    #[test]
    fn variant_sets_bind_to_the_path() {
        let field = field("store field f @ 0 requires a.b.c.{X, Y} & d.e.f.On");

        let space = field.requires.plain.unwrap().inner;
        assert_eq!(space.patterns.len(), 1);

        let pattern = &space.patterns[0].inner;
        assert_eq!(pattern.entitlements.len(), 2);
        assert_eq!(pattern.entitlements[0].inner.set.as_ref().unwrap().len(), 2);
        assert!(pattern.entitlements[1].inner.set.is_none());
    }

    #[test]
    fn correspondence_brackets_bind_to_segments() {
        let field = field("store field f @ 0 requires dma.ccr[0..8].en.Disabled");

        let space = field.requires.plain.unwrap().inner;
        let entitled = &space.patterns[0].inner.entitlements[0].inner;

        assert_eq!(entitled.segments.len(), 4);
        assert!(entitled.segments[0].inner.elements.is_none());
        assert!(entitled.segments[1].inner.elements.is_some());
        assert!(entitled.set.is_none());
    }

    #[test]
    fn correspondences_and_sets_compose() {
        let field = field("store field f @ 0 requires dma.ccr[0..8].psize.{Bits8, Bits16}");

        let space = field.requires.plain.unwrap().inner;
        let entitled = &space.patterns[0].inner.entitlements[0].inner;

        assert_eq!(entitled.segments.len(), 3);
        assert!(entitled.segments[1].inner.elements.is_some());
        assert_eq!(entitled.set.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn requirement_spaces_are_disjunctions_of_patterns() {
        let field = field(
            "store field f @ 0
                requires (a.b.c.On & d.e.f.On) | g.h.i.Off",
        );

        let space = field.requires.plain.unwrap().inner;
        assert_eq!(space.patterns.len(), 2);
        assert_eq!(space.patterns[0].inner.entitlements.len(), 2);
        assert_eq!(space.patterns[1].inner.entitlements.len(), 1);
    }
}

mod variants {
    use super::*;

    fn variants(src: &str) -> Vec<crate::ast::Variant<'_>> {
        match file(src).items.into_iter().next().unwrap().inner {
            FileItem::Field(field) => field
                .body
                .unwrap()
                .inner
                .into_iter()
                .filter_map(|item| match item.inner {
                    crate::ast::FieldItem::Variant(variant) => Some(variant),
                    crate::ast::FieldItem::Error => None,
                })
                .collect(),
            _ => panic!("a field"),
        }
    }

    #[test]
    fn sides_are_optional_single_words() {
        let variants = variants(
            "read write field f @ 0..=1 {
                read variant Busy ~ 1
                write variant Go ~ 2
                variant Idle ~ 0
            }",
        );

        assert!(matches!(variants[0].side.unwrap().inner, Side::Read));
        assert!(matches!(variants[1].side.unwrap().inner, Side::Write));
        assert!(variants[2].side.is_none());
    }

    #[test]
    fn modifiers_precede_the_side() {
        let variants = variants(
            "read write field f @ 0 {
                leaky inert read variant Idle ~ 0
            }",
        );

        assert!(variants[0].leaky.is_some());
        assert!(variants[0].inert.is_some());
        assert!(variants[0].side.is_some());
    }

    #[test]
    fn arrays_take_value_lists_with_strides() {
        let variants = variants(
            "store field f @ 0..=5 {
                variant array P[1..=15] ~ [4, ...+4]
            }",
        );

        assert!(variants[0].array.is_some());
        assert!(matches!(
            variants[0].value.as_ref().unwrap().inner,
            VariantValue::List(..),
        ));
    }

    #[test]
    fn only_read_and_write_are_sides() {
        // `store` is an access modality, not a side — contextual operators
        assert!(errors("store field f @ 0 { store variant A ~ 0 }") > 0);
    }
}

mod schemas {
    use super::*;

    #[test]
    fn schemas_have_no_access_modality() {
        let file = file(
            "schema enable {
                variant Disabled ~ 0
                read variant Ready ~ 1
            }",
        );

        assert!(matches!(file.items[0].inner, FileItem::Schema(..)));
    }

    #[test]
    fn access_words_before_schema_are_rejected() {
        assert!(errors("store schema enable { variant On ~ 1 }") > 0);
    }
}

mod registers {
    use super::*;

    #[test]
    fn registers_have_no_requires() {
        // requires is a peripheral, field, and variant property — contextual
        assert!(errors("register r @ 0x0 requires a.b.c.On") > 0);
    }

    #[test]
    fn designator_series_parse() {
        let file = file(
            "register array r[0..=3] @ [0x0, ...] { read field array f[0..=7, ...] @ [0, ...] }",
        );

        match &file.items[0].inner {
            FileItem::Register(register) => {
                assert!(matches!(
                    register.head.indices.as_ref().unwrap().inner,
                    crate::ast::Indices::Range(..),
                ));
            }
            _ => panic!("a register"),
        }
    }

    #[test]
    fn arrays_auto_increment() {
        let file = file("register array r[0..=3] @ [0x0, ...]");

        match &file.items[0].inner {
            FileItem::Register(register) => assert!(register.array.is_some()),
            _ => panic!("a register"),
        }
    }
}

mod devices {
    use super::*;

    #[test]
    fn the_kitchen_sink() {
        file(
            "import st.gpio

            /// A demonstration.
            device demo {
                schema #enable

                peripheral group gpio {
                    peripheral array #gpio.port as gpio[a, b] @ [0x4800_0000, 0x4800_0400]
                }

                leaky peripheral wwdg @ 0x4000_2c00
                    requires rcc.apb1enr.wwdgen.Enabled
                {
                    register cr @ 0x0 reset 0x7f {
                        store field t @ 0..=6 reset 0x7f
                    }
                }

                interrupts {
                    wwdg
                    reserved
                    rtc
                }
            }",
        );
    }

    #[test]
    fn imports_are_paths() {
        let file = file("import st.g4.gpio");

        match &file.items[0].inner {
            FileItem::Import(import) => assert_eq!(import.path.inner.segments.len(), 3),
            _ => panic!("an import"),
        }
    }
}

mod docs {
    use super::*;

    #[test]
    fn doc_comments_attach() {
        let file = file(
            "/// A basic enable switch.
            /// Two lines of it.
            schema enable { variant On ~ 1 }",
        );

        match &file.items[0].inner {
            FileItem::Schema(schema) => assert_eq!(schema.docs.len(), 2),
            _ => panic!("a schema"),
        }
    }
}

mod recovery {
    use super::*;

    #[test]
    fn garbage_does_not_take_siblings_with_it() {
        let (file, errors) = parse(
            "register good_a @ 0x0
            ??? what even is this ???
            register good_b @ 0x4",
            0,
        );

        let file = file.expect("recovery keeps the file");
        let registers = file
            .items
            .iter()
            .filter(|item| matches!(item.inner, FileItem::Register(..)))
            .count();

        assert_eq!(registers, 2, "both healthy items survive");
        assert!(!errors.is_empty());
    }

    #[test]
    fn every_broken_item_is_reported() {
        assert!(
            errors(
                "register @ ??
                field ~~ nope",
            ) >= 2,
            "multiple diagnostics, not just the first",
        );
    }
}

mod spans {
    use super::*;

    #[test]
    fn spans_carry_their_source() {
        let (.., errors) = parse("register r @ 0xzz", 7);

        match &errors[0] {
            crate::Error::Parse(error) => assert_eq!(error.span().context, 7),
            crate::Error::Lex(error) => assert_eq!(error.span().context, 7),
        }
    }

    #[test]
    fn item_spans_carry_their_source() {
        let (file, ..) = parse("register r @ 0x0", 3);

        assert_eq!(file.unwrap().items[0].span.context, 3);
    }
}
