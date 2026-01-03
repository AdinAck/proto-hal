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

/// A requirement for a particular field to inhabit a particular state.
///
/// An entitlement is **satisfied** when a hardware state fulfills the requirement.
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

/// A set of [`Entitlement`]s, grouped by field.
///
/// A pattern is **satisfied** when *all* fields contain a satisfied entitlement.
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
    /// ## Meaning
    /// When a pattern is valid, it means that there exists *at least* one combination of states which satisfies the
    /// pattern.
    ///
    /// ## Definition
    /// A pattern is **valid** if it does not contradict the ontological entitlement spaces of its fields, nor the
    /// statewise entitlement spaces of its states.
    pub fn validate(&self, model: &Model) -> Result<(), PatternError> {
        Search::new(model).validate_pattern(self)
    }

    /// Determine if the pattern contradicts a [`Space`] or not.
    ///
    /// ## Meaning
    /// When a pattern contradicts a space, it means that there **does not exist** a combination of states which
    /// satisfies both the pattern *and* the space.
    ///
    /// ## Definition
    /// A pattern contradicts a space if it contradicts **all** patterns in the space.
    pub fn contradicts_space(&self, model: &Model, space: &Space) -> bool {
        Search::new(model).pattern_contradicts_space(self, space)
    }

    /// Determine if the pattern contradicts another or not.
    ///
    /// ## Meaning
    /// When a pattern contradicts another, it means that there **does not exist** a combination of states which
    /// satisfies both patterns.
    ///
    /// ## Definition
    /// A pattern *trivially* contradicts another if there exists a field where the set of states for that field in each
    /// pattern are disjoint.
    ///
    /// A pattern contradicts another if *every* possible combination of states (represented as a patern itself):
    /// 1. trivially contradicts with either pattern *or*
    /// 2. contradicts the ontological entitlement space of any field in the combination *or*
    /// 3. contradicts the statewise entitlement space of any state in the combination
    pub fn contradicts_pattern(&self, model: &Model, other: &Pattern) -> bool {
        Search::new(model).pattern_contradicts_pattern(self, other)
    }

    /// Determine if this pattern covers another or not.
    ///
    /// ## Meaning
    /// When a pattern covers another, it means that the other pattern is **extraneous**. In other words, if the two
    /// patterns were to reside within the same space, the covered pattern would not affect the properties of the space
    /// in any way. It is as if the pattern is not present.
    ///
    /// ## Definition
    /// A pattern covers another if its fields are a subset of the other pattern's fields and for each matching field
    /// pair, the state sets are a superset of the counterpart.
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

    /// The entitlements in this pattern.
    pub fn entitlements<'a>(&'a self) -> impl Iterator<Item = &'a Entitlement> {
        self.entitlements.values().flat_map(|field| field.iter())
    }

    pub fn fields<'cx>(&'cx self, model: &'cx Model) -> impl Iterator<Item = View<'cx, FieldNode>> {
        self.entitlements
            .keys()
            .map(|&field_index| model.get_field(field_index))
    }

    pub fn render(&self, model: &Model) -> String {
        self.entitlements
            .iter()
            .map(|(&field_index, entitlements)| {
                let entitlement_idents = entitlements
                    .iter()
                    .map(|entitlement| entitlement.variant(model).type_name().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "<{} | {}>",
                    model.get_field(field_index).module_name().to_string(),
                    entitlement_idents
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
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

/// The error that may be emitted when validating a pattern.
#[derive(Debug, Clone)]
pub enum PatternError {
    Invalid, // TODO: eventually should describe *why* the pattern is invalid
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deref, DerefMut, AsRef, AsMut)]
struct Combination(Pattern);

impl Combination {
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

        if !intersection.values().any(|s| s.is_empty()) {
            // if the patterns don't trivially contradict, deep search

            for combination in Self::combinations(&intersection) {
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

            let children = inner(entitlements, field_indices.clone());

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

            result
        }

        inner(entitlements, entitlements.keys())
    }
}

/// A space is a set of [`Pattern`]s.
///
/// A space is **satisfied** when *any* of its [`Pattern`]s are satisfied.
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

    /// Create a space from the nested iterator where each nest is a pattern.
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

    /// The [`Pattern`]s in the space.
    pub fn patterns<'a>(&'a self) -> impl Iterator<Item = &'a Pattern> {
        self.patterns.iter()
    }

    /// The [`FieldIndex`]s in the space.
    ///
    /// *Note: Field indicies may be produced more than once.*
    pub fn field_indicies<'a>(&'a self) -> impl Iterator<Item = &'a FieldIndex> {
        self.patterns()
            .flat_map(|pattern| pattern.entitlements.keys())
    }

    /// The [`Entitlement`]s in the space.
    pub fn entitlements<'a>(&'a self) -> impl Iterator<Item = &'a Entitlement> {
        self.patterns().flat_map(|pattern| pattern.entitlements())
    }

    /// The number of [`Pattern`]s in the space.
    pub fn count(&self) -> usize {
        self.patterns.len()
    }
}

impl<'cx> View<'cx, Space> {
    /// Determine whether the space contradicts the other or not.
    ///
    /// ## Meaning
    /// When a space contradicts another, it means that there **does not exist** a combination of states which
    /// satisfies both spaces.
    ///
    /// ## Definition
    /// A space contradicts another if **all** [`Pattern`]s in the space contradict the other space.
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
