use std::hash::Hash;

use derive_more::{AsMut, AsRef, Deref, DerefMut};
use indexmap::{IndexMap, IndexSet};
use proc_macro2::TokenStream;
use quote::quote;

use crate::{
    Node,
    diagnostic::Context,
    field::{FieldIndex, FieldNode},
    model::{Model, View},
    peripheral::PeripheralIndex,
    variant::{VariantIndex, VariantNode},
};

/// An entitlement represents a field inhabiting a particular state.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Entitlement(pub(crate) VariantIndex);

impl Entitlement {
    pub(crate) fn index(&self) -> VariantIndex {
        self.0
    }

    pub fn variant<'cx>(&self, model: &'cx Model) -> View<'cx, VariantNode> {
        model.get_variant(self.0)
    }

    pub fn field<'cx>(&self, model: &'cx Model) -> View<'cx, FieldNode> {
        model.get_field(self.variant(model).parent)
    }

    pub fn render_up_to_field(&self, model: &Model) -> TokenStream {
        let field = self.field(model);
        let register = model.get_register(field.parent);
        let peripheral = model.get_peripheral(register.parent.clone());

        let peripheral_ident = peripheral.module_name();
        let register_ident = register.module_name();
        let field_ident = field.module_name();

        quote! {
            #peripheral_ident::#register_ident::#field_ident
        }
    }

    pub fn render_entirely(&self, model: &Model) -> TokenStream {
        let prefix = self.render_up_to_field(model);
        let variant = self.variant(model);

        let variant_ident = variant.type_name();

        quote! { #prefix::#variant_ident }
    }

    pub fn to_string(&self, model: &Model) -> String {
        self.render_entirely(model)
            .to_string()
            .split_whitespace()
            .collect()
    }
}

/// A set of [`Entitlement`]s. A pattern is considered *satisfied* if **all** of its entitlements are satisfied.
///
/// A pattern is valid if all of its entitlements have inseperable statewise and ontological entitlement spaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    entitlements: IndexMap<FieldIndex, IndexSet<Entitlement>>,
}

impl Pattern {
    /// Create a pattern with the provided entitlements.
    pub fn new(
        model: &Model,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) -> Result<Self, PatternError> {
        let mut entitlement_map = IndexMap::<FieldIndex, IndexSet<Entitlement>>::new();

        for entitlement in entitlements {
            entitlement_map
                .entry(entitlement.field(model).index)
                .or_default()
                .insert(entitlement);
        }

        let this = Self {
            entitlements: entitlement_map,
        };

        this.validate(model)?;

        Ok(this)
    }

    /// Validate the pattern.
    ///
    /// TODO: Explain what makes a pattern valid.
    pub fn validate(&self, model: &Model) -> Result<(), PatternError> {
        Search::new(model).validate_pattern(self)
    }

    /// Determine if the pattern contradicts a space or not.
    ///
    /// TODO: Explain how this is known.
    pub fn contradicts_space(&self, model: &Model, space: &Space) -> bool {
        Search::new(model).pattern_contradicts_space(self, space)
    }

    /// Determine if the pattern contradicts another or not.
    ///
    /// TODO: Explain how this is known.
    pub fn contradicts_pattern(&self, model: &Model, other: &Pattern) -> bool {
        Search::new(model).pattern_contradicts_pattern(self, other)
    }

    /// Determine if this pattern covers another or not.
    pub fn covers(&self, other: &Self) -> bool {
        self.entitlements
            .keys()
            .all(|field| other.entitlements.contains_key(field))
            && self
                .entitlements
                .iter()
                .filter_map(|(field, lhs)| other.entitlements.get(field).map(|rhs| (lhs, rhs)))
                .all(|(lhs, rhs)| rhs.difference(lhs).next().is_none())
    }

    pub fn entitlements<'a>(&'a self) -> impl Iterator<Item = &'a Entitlement> {
        self.entitlements.values().flat_map(|field| field.iter())
    }

    fn intersection_with(&self, other: &Self) -> IndexMap<FieldIndex, IndexSet<Entitlement>> {
        let mut intersection = IndexMap::new();

        let fields = self
            .entitlements
            .keys()
            .chain(other.entitlements.keys())
            .collect::<IndexSet<_>>();

        for field in fields {
            match (self.entitlements.get(field), other.entitlements.get(field)) {
                (None, None) => continue,
                (None, Some(states)) | (Some(states), None) => {
                    intersection.insert(*field, states.clone());
                }
                (Some(lhs), Some(rhs)) => {
                    intersection.insert(*field, lhs.intersection(rhs).cloned().collect());
                }
            }
        }

        intersection
    }
}

#[derive(Debug)]
pub enum PatternError {
    Invalid,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deref, DerefMut, AsRef, AsMut)]
struct Combination(Pattern);

impl Combination {
    fn new(model: &Model, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        Self(Pattern {
            entitlements: IndexMap::from_iter(entitlements.into_iter().map(|entitlement| {
                (
                    entitlement.field(model).index,
                    IndexSet::from([entitlement]),
                )
            })),
        })
    }

    fn insert(&mut self, field: FieldIndex, entitlement: Entitlement) {
        self.0
            .entitlements
            .insert(field, IndexSet::from([entitlement]));
    }
}

struct Search<'cx> {
    model: &'cx Model,
    /// The patterns which have already been traversed for validation.
    seen: Vec<Pattern>,
}

impl<'cx> Search<'cx> {
    fn new(model: &'cx Model) -> Self {
        Self {
            model,
            seen: Default::default(),
        }
    }

    fn validate_pattern(&mut self, pattern: &Pattern) -> Result<(), PatternError> {
        if self.seen.contains(pattern) {
            return Ok(());
        }

        self.seen.push(pattern.clone());

        if pattern
            .entitlements
            .keys()
            .filter_map(|&field_index| self.model.get_field(field_index).ontological_entitlements())
            .chain(
                pattern
                    .entitlements
                    .values()
                    .flatten()
                    .filter_map(|entitlement| {
                        entitlement.variant(self.model).statewise_entitlements()
                    }),
            )
            .any(|space| self.pattern_contradicts_space(pattern, &space))
        {
            Err(PatternError::Invalid)?
        }

        Ok(())
    }

    fn pattern_contradicts_space(&mut self, pattern: &Pattern, space: &Space) -> bool {
        space
            .patterns
            .iter()
            .all(|other| self.pattern_contradicts_pattern(pattern, other))
    }

    fn pattern_contradicts_pattern(&mut self, lhs: &Pattern, rhs: &Pattern) -> bool {
        let intersection = lhs.intersection_with(rhs);
        println!("intersection: {intersection:?}");
        if !intersection.values().any(|s| s.is_empty()) {
            // if the patterns don't trivially contradict, deep search

            for combination in Self::combinations(&intersection) {
                println!("combination: {combination:?}");
                if self.validate_pattern(&combination).is_ok() {
                    return false;
                }
            }
        }

        true
    }

    // this function was conceived in its entirety by Peter Gao
    fn combinations(
        entitlements: &IndexMap<FieldIndex, IndexSet<Entitlement>>,
    ) -> Vec<Combination> {
        fn inner<'a>(
            entitlements: &IndexMap<FieldIndex, IndexSet<Entitlement>>,
            mut field_indices: impl Iterator<Item = &'a FieldIndex> + Clone,
        ) -> Vec<Combination> {
            let Some(field_index) = field_indices.next() else {
                return vec![Default::default()];
            };

            println!("field_index: {field_index:?}");

            let children = inner(entitlements, field_indices.clone());
            println!("children: {children:?}");
            let result = entitlements
                .get(field_index)
                .expect("field index must exist")
                .iter()
                .flat_map(|entitlement| {
                    children.iter().cloned().map(|mut child| {
                        child.insert(*field_index, *entitlement);

                        child
                    })
                })
                .collect();
            println!("result: {result:?}");
            result
        }

        inner(entitlements, entitlements.keys())
    }
}

/// A space is a set of [`Pattern`]s. A space is considered *satisfied* if **any** of
/// its patterns are satisfied.
#[derive(Debug, Clone, Default)]
pub struct Space {
    patterns: Vec<Pattern>,
}

impl Space {
    /// Create a space from the provided patterns.
    pub fn new(patterns: impl Iterator<Item = Pattern>) -> Self {
        let mut pattern_list = vec![];

        for pattern in patterns {
            pattern_list = pattern_list
                .into_iter()
                .filter(|existing| !pattern.covers(existing))
                .collect();

            if !pattern_list
                .iter()
                .any(|existing| existing.covers(&pattern))
            {
                pattern_list.push(pattern);
            }
        }

        Self {
            patterns: pattern_list,
        }
    }

    pub fn from_iter(
        model: &Model,
        entitlements: impl IntoIterator<Item = impl IntoIterator<Item = Entitlement>>,
    ) -> Result<Self, PatternError> {
        Ok(Self::new(
            entitlements
                .into_iter()
                .map(|entitlements| Pattern::new(model, entitlements.into_iter()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter(),
        ))
    }

    pub fn patterns<'a>(&'a self) -> impl Iterator<Item = &'a Pattern> {
        self.patterns.iter()
    }

    pub fn entitlements<'a>(&'a self) -> impl Iterator<Item = &'a Entitlement> {
        self.patterns().flat_map(|pattern| pattern.entitlements())
    }

    /// The number of patterns in the space.
    pub fn count(&self) -> usize {
        self.patterns.len()
    }
}

impl<'cx> View<'cx, Space> {
    /// Determine whether the spaces contradict or not.
    ///
    /// Spaces contradict if for any combination of states, at most one space is satisfied.
    pub fn contradicts(&self, other: &Space) -> bool {
        self.patterns
            .iter()
            .all(|pattern| pattern.contradicts_space(self.model, other))
    }
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
    use crate::{Entitlement, Field, Model, Peripheral, Register, Variant, entitlement::Pattern};

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
                let mut model = Model::new();

                let mut p = model.add_peripheral(Peripheral::new("p", 0));
                let mut r = p.add_register(Register::new("r", 0));

                let mut f0 = r.add_store_field(Field::new("f1", 0, 1));

                let f0_e0 = f0.add_variant(Variant::new("State0", 0)).make_entitlement();
                let f0_e1 = f0.add_variant(Variant::new("State1", 1)).make_entitlement();

                let mut f1 = r.add_store_field(Field::new("f2", 0, 1));

                let mut f1_e0 = f1.add_variant(Variant::new("State0", 0));
                f1_e0
                    .statewise_entitlements([[f0_e0]])
                    .expect("expected statewise entitlement space to be valid");
                let f1_e0 = f1_e0.make_entitlement();
                let f1_e1 = f1.add_variant(Variant::new("State1", 1)).make_entitlement();

                Setup {
                    model,
                    f0_e0,
                    f0_e1,
                    f1_e0,
                    f1_e1,
                }
            }

            #[test]
            fn statewise_violation() {
                let setup = setup();

                assert!(Pattern::new(&setup.model, [setup.f0_e1, setup.f1_e0]).is_err());
            }

            #[test]
            fn statewise_adherence() {
                let setup = setup();

                assert!(Pattern::new(&setup.model, [setup.f0_e0, setup.f1_e1]).is_ok());
                assert!(Pattern::new(&setup.model, [setup.f0_e0, setup.f1_e0]).is_ok());
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
                let mut model = Model::new();

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
            fn suprset() {
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
    }
}
