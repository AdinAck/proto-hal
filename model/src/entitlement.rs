pub mod codegen;
pub mod pattern;
mod search;
pub mod space;

use std::hash::Hash;

use crate::{
    Node,
    diagnostic::Context,
    field::{FieldIndex, FieldNode},
    model::{Model, View},
    peripheral::PeripheralIndex,
    variant::{VariantIndex, VariantNode},
};

pub use pattern::Pattern;
pub use space::Space;

/// A requirement for a particular field to inhabit a particular state.
///
/// An entitlement is **satisfied** when a hardware state fulfills the requirement.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Entitlement(pub(crate) VariantIndex);

impl Entitlement {
    pub fn variant<'cx>(&self, model: &'cx Model) -> View<'cx, VariantNode> {
        model.get_variant(self.0)
    }

    pub fn field<'cx>(&self, model: &'cx Model) -> View<'cx, FieldNode> {
        model.get_field(self.variant(model).parent)
    }

    pub fn to_string(&self, model: &Model) -> String {
        self.render_entirely(model)
            .to_string()
            .split_whitespace()
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Axis {
    Statewise,
    Affordance,
    Ontological,
}

impl Node for Space {
    type Index = EntitlementIndex;
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum EntitlementIndex {
    Peripheral(PeripheralIndex),
    Field(FieldIndex),
    Write(FieldIndex),
    HardwareWrite(FieldIndex),
    Variant(VariantIndex),
}

impl EntitlementIndex {
    pub fn into_context(&self, model: &Model) -> Context {
        Context::with_path(match self {
            EntitlementIndex::Peripheral(peripheral_index) => {
                vec![
                    model
                        .get_peripheral(peripheral_index.clone())
                        .module_name()
                        .to_string(),
                ]
            }
            EntitlementIndex::Field(field_index)
            | EntitlementIndex::Write(field_index)
            | EntitlementIndex::HardwareWrite(field_index) => {
                let field = model.get_field(*field_index);
                let register = model.get_register(field.parent);
                let peripheral = model.get_peripheral(register.parent.clone());

                vec![
                    peripheral.module_name().to_string(),
                    register.module_name().to_string(),
                    field.module_name().to_string(),
                ]
            }
            EntitlementIndex::Variant(variant_index) => {
                let variant = model.get_variant(*variant_index);
                let field = model.get_field(variant.parent);
                let register = model.get_register(field.parent);
                let peripheral = model.get_peripheral(register.parent.clone());

                vec![
                    peripheral.module_name().to_string(),
                    register.module_name().to_string(),
                    field.module_name().to_string(),
                    variant.module_name().to_string(),
                ]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Composition, Entitlement, Field, Model, Peripheral, Register, Variant,
        entitlement::{Pattern, Space},
    };

    mod patterns {
        use super::*;

        mod validation {
            use super::*;

            struct Setup {
                model: Model,
                f0_e0: Entitlement,
                f0_e1: Entitlement,
                f1_e0: Entitlement,
                f1_e1: Entitlement,
            }

            fn setup() -> Setup {
                let mut model = Composition::new();

                let mut p = model.add_peripheral(Peripheral::new("p", 0));
                let mut r = p.add_register(Register::new("r", 0));

                let mut f0 = r.add_store_field(Field::new("f1", 0, 1));

                let f0_e0 = f0.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f0_e1 = f0.add_variant(Variant::new("State1", 1)).make_entitlement();

                let mut f1 = r.add_store_field(Field::new("f2", 0, 1));

                let mut f1_e0 = f1.add_variant(Variant::new("State0", 0));
                f1_e0.statewise_entitlements([[f0_e0]]);
                let f1_e0 = f1_e0.make_entitlement();
                let f1_e1 = f1.add_variant(Variant::new("State1", 1)).make_entitlement();

                let model = model.release(); // this model doesn't need to be entirely valid

                Setup {
                    model,
                    f0_e0,
                    f0_e1,
                    f1_e0,
                    f1_e1,
                }
            }

            #[test]
            fn statewise_adherence() {
                let setup = setup();

                // none of these states have any requirements
                assert!(Pattern::new(&setup.model, [setup.f0_e0, setup.f1_e1]).is_ok());
                assert!(Pattern::new(&setup.model, [setup.f0_e1, setup.f1_e1]).is_ok());
                // the requirement is satisfied exactly
                assert!(Pattern::new(&setup.model, [setup.f0_e0, setup.f1_e0]).is_ok());
                // since this pattern doesn't specify f0 at all, it *is* possible for f0 to inhabit f0_e0
                assert!(Pattern::new(&setup.model, [setup.f1_e0]).is_ok());
                // this pattern is identical to the previous since f0 is exhaustive
                assert!(
                    Pattern::new(&setup.model, [setup.f1_e0, setup.f0_e0, setup.f0_e1]).is_ok()
                );
            }

            #[test]
            fn statewise_violation() {
                let setup = setup();

                // this pattern is impossible because f1_e0 *requires* f0_e0, but this pattern requires f0 to inhabit
                // f0_e1 which is !f0_e0, and since a field can only inhabit one state at a time, the pattern is
                // impossible
                assert!(Pattern::new(&setup.model, [setup.f0_e1, setup.f1_e0]).is_err());
            }
        }

        mod contradiction {
            use super::*;

            struct Setup {
                model: Model,
                pat0: Pattern,
                f0_e0: Entitlement,
                f0_e1: Entitlement,
                f1_e0: Entitlement,
                f1_e1: Entitlement,
                f1_e2: Entitlement,
                f2_e1: Entitlement,
            }

            fn setup() -> Setup {
                let mut model = Composition::new();

                let mut p = model.add_peripheral(Peripheral::new("p", 0));
                let mut r = p.add_register(Register::new("r", 0));

                let mut f0 = r.add_store_field(Field::new("f1", 0, 1));

                let f0_e0 = f0.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f0_e1 = f0.add_variant(Variant::new("State1", 1)).make_entitlement();

                let mut f1 = r.add_store_field(Field::new("f2", 0, 2));

                let f1_e0 = f1.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f1_e1 = f1.add_variant(Variant::new("State1", 1)).make_entitlement();
                let f1_e2 = f1.add_variant(Variant::new("State2", 2)).make_entitlement();

                let mut f2 = r.add_store_field(Field::new("f2", 0, 2));

                f2.add_variant(Variant::new("State0", 0));
                let f2_e1 = f2.add_variant(Variant::new("State1", 1)).make_entitlement();
                f2.add_variant(Variant::new("State1", 1));

                let pat0 = Pattern::new(&model, [f0_e0, f1_e1, f1_e2])
                    .expect("expected pattern to be valid");

                let model = model.release(); // this model doesn't need to be entirely valid

                Setup {
                    model,
                    pat0,
                    f0_e0,
                    f0_e1,
                    f1_e0,
                    f1_e1,
                    f1_e2,
                    f2_e1,
                }
            }

            #[test]
            fn single_field() {
                let setup = setup();

                let pat1 = Pattern::new(&setup.model, [setup.f1_e0, setup.f0_e0])
                    .expect("expected pattern to be valid");

                assert!(setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn two_field() {
                let setup = setup();

                let pat1 = Pattern::new(&setup.model, [setup.f1_e0, setup.f0_e1])
                    .expect("expected pattern to be valid");

                assert!(setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn subset() {
                let setup = setup();

                let pat1 = Pattern::new(&setup.model, [setup.f1_e1, setup.f0_e0])
                    .expect("expected pattern to be valid");

                assert!(!setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn superset() {
                let setup = setup();

                let pat1 = Pattern::new(
                    &setup.model,
                    [setup.f1_e1, setup.f1_e2, setup.f0_e1, setup.f0_e0],
                )
                .expect("expected pattern to be valid");

                assert!(!setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn partial_overlap() {
                let setup = setup();

                let pat1 = Pattern::new(
                    &setup.model,
                    [setup.f1_e0, setup.f1_e1, setup.f0_e1, setup.f0_e0],
                )
                .expect("expected pattern to be valid");

                assert!(!setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn wildcard() {
                let setup = setup();

                let pat1 = Pattern::new(&setup.model, [setup.f1_e1])
                    .expect("expected pattern to be valid");

                assert!(!setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }

            #[test]
            fn two_wildcard() {
                let setup = setup();

                let pat1 = Pattern::new(&setup.model, [setup.f1_e1, setup.f2_e1])
                    .expect("expected pattern to be valid");

                assert!(!setup.pat0.contradicts_pattern(&setup.model, &pat1));
            }
        }

        mod complement {
            use super::*;

            struct Setup {
                model: Model,
                pat0: Pattern,
                f0_e0: Entitlement,
                f0_e1: Entitlement,
                f1_e0: Entitlement,
            }

            fn setup() -> Setup {
                let mut model = Composition::new();

                let mut p = model.add_peripheral(Peripheral::new("p", 0));
                let mut r = p.add_register(Register::new("r", 0));

                let mut f0 = r.add_store_field(Field::new("f1", 0, 1));

                let f0_e0 = f0.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f0_e1 = f0.add_variant(Variant::new("State1", 1)).make_entitlement();

                let mut f1 = r.add_store_field(Field::new("f2", 0, 2));

                let f1_e0 = f1.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f1_e1 = f1.add_variant(Variant::new("State1", 1)).make_entitlement();
                let f1_e2 = f1.add_variant(Variant::new("State2", 2)).make_entitlement();

                let pat0 = Pattern::new(&model, [f0_e0, f1_e1, f1_e2])
                    .expect("expected pattern to be valid");

                let model = model.release(); // this model doesn't need to be entirely valid

                Setup {
                    model,
                    pat0,
                    f0_e0,
                    f0_e1,
                    f1_e0,
                }
            }

            #[test]
            fn simple() {
                let setup = setup();

                let complement = setup.pat0.complement(&setup.model);

                assert!(
                    complement
                        .patterns
                        .contains(&Pattern::new(&setup.model, [setup.f0_e1]).unwrap())
                );
                assert!(
                    complement
                        .patterns
                        .contains(&Pattern::new(&setup.model, [setup.f1_e0]).unwrap())
                );
                assert_eq!(complement.count(), 2);
            }

            // an empty pattern is a tuatology (always satisfied), so the complement
            // is an empty space which is a contradiction (never satisfied)
            #[test]
            fn empty() {
                let setup = setup();

                let complement = Pattern::new(&setup.model, [])
                    .unwrap()
                    .complement(&setup.model);

                assert!(complement.patterns.is_empty());
            }

            // an exhaustive pattern is a tuatology (always satisfied), so the
            // complement is an empty space which is a contradiction (never
            // satisfied)
            #[test]
            fn exhaustive() {
                let setup = setup();

                let complement = Pattern::new(&setup.model, [setup.f0_e0, setup.f0_e1])
                    .unwrap()
                    .complement(&setup.model);

                assert!(complement.patterns.is_empty());
            }
        }
    }

    mod spaces {
        use super::*;

        mod complement {
            use super::*;

            struct Setup {
                model: Model,
                pat0: Pattern,
                f0_e0: Entitlement,
                f0_e1: Entitlement,
                f1_e0: Entitlement,
                f1_e1: Entitlement,
                f1_e2: Entitlement,
                f2_e0: Entitlement,
                f2_e1: Entitlement,
                f2_e2: Entitlement,
            }

            fn setup() -> Setup {
                let mut model = Composition::new();

                let mut p = model.add_peripheral(Peripheral::new("p", 0));
                let mut r = p.add_register(Register::new("r", 0));

                let mut f0 = r.add_store_field(Field::new("f1", 0, 1));

                let f0_e0 = f0.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f0_e1 = f0.add_variant(Variant::new("State1", 1)).make_entitlement();

                let mut f1 = r.add_store_field(Field::new("f2", 0, 2));

                let f1_e0 = f1.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f1_e1 = f1.add_variant(Variant::new("State1", 1)).make_entitlement();
                let f1_e2 = f1.add_variant(Variant::new("State2", 2)).make_entitlement();

                let mut f2 = r.add_store_field(Field::new("f2", 0, 2));

                let f2_e0 = f2.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f2_e1 = f2.add_variant(Variant::new("State1", 1)).make_entitlement();
                let f2_e2 = f2.add_variant(Variant::new("State1", 1)).make_entitlement();

                let pat0 = Pattern::new(&model, [f0_e0, f1_e1, f1_e2])
                    .expect("expected pattern to be valid");

                let model = model.release(); // this model doesn't need to be entirely valid

                Setup {
                    model,
                    pat0,
                    f0_e0,
                    f0_e1,
                    f1_e0,
                    f1_e1,
                    f1_e2,
                    f2_e0,
                    f2_e1,
                    f2_e2,
                }
            }

            #[test]
            fn single() {
                let setup = setup();

                let space = Space::new([setup.pat0.clone()]);
                let complement = space.complement(&setup.model);

                assert_eq!(complement, setup.pat0.complement(&setup.model));
            }

            #[test]
            fn multiple_nonoverlapping() {
                let setup = setup();

                let space = Space::new([
                    setup.pat0,
                    Pattern::new(&setup.model, [setup.f2_e1]).unwrap(),
                ]);
                let complement = space.complement(&setup.model);

                assert!(complement.patterns.contains(
                    &Pattern::new(&setup.model, [setup.f0_e1, setup.f2_e0, setup.f2_e2]).unwrap()
                ));
                assert!(complement.patterns.contains(
                    &Pattern::new(&setup.model, [setup.f1_e0, setup.f2_e0, setup.f2_e2]).unwrap()
                ));
                assert_eq!(complement.count(), 2);
            }

            #[test]
            fn multiple_overlapping() {
                let setup = setup();

                let space = Space::new([
                    setup.pat0,
                    Pattern::new(&setup.model, [setup.f0_e0, setup.f2_e1]).unwrap(),
                ]);
                let complement = space.complement(&setup.model);

                assert!(
                    complement
                        .patterns
                        .contains(&Pattern::new(&setup.model, [setup.f0_e1]).unwrap())
                );
                assert!(complement.patterns.contains(
                    &Pattern::new(&setup.model, [setup.f1_e0, setup.f2_e0, setup.f2_e2]).unwrap()
                ));
                assert_eq!(complement.count(), 2);
            }

            #[test]
            fn multiple_contradicting() {
                let setup = setup();

                let space = Space::new([
                    setup.pat0,
                    Pattern::new(&setup.model, [setup.f0_e1, setup.f1_e0]).unwrap(),
                ]);
                let complement = space.complement(&setup.model);

                assert!(complement.patterns.contains(
                    &Pattern::new(&setup.model, [setup.f0_e1, setup.f1_e1, setup.f1_e2]).unwrap()
                ));
                assert!(
                    complement
                        .patterns
                        .contains(&Pattern::new(&setup.model, [setup.f0_e0, setup.f1_e0]).unwrap())
                );
                assert_eq!(complement.count(), 2);
            }

            #[test]
            fn empty() {
                let setup = setup();

                let space = Space::contradiction();
                let complement = space.complement(&setup.model);

                assert!(complement.patterns.contains(&Pattern::tautology()));
                assert_eq!(complement.count(), 1);
            }

            #[test]
            fn exhaustive() {
                let setup = setup();

                for space in [
                    Space::new([Pattern::tautology()]),
                    Space::new([Pattern::new(&setup.model, [setup.f0_e0, setup.f0_e1]).unwrap()]),
                ] {
                    println!("{space:?}");
                    let complement = space.complement(&setup.model);
                    println!("{complement:?}");
                    assert_eq!(complement.count(), 0);
                }
            }
        }
    }
}
