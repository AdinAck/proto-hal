use indexmap::{IndexMap, IndexSet};
use proc_macro2::TokenStream;
use quote::quote;
use ters::ters;

use crate::{
    Node,
    diagnostic::Context,
    field::{FieldIndex, FieldNode},
    model::{Model, View},
    peripheral::PeripheralIndex,
    variant::{VariantIndex, VariantNode},
};

/// An entitlement represents a field inhabiting a particular state.
#[ters]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Entitlement {
    #[get]
    field_index: FieldIndex,
    #[get]
    variant_index: VariantIndex,
}

impl Entitlement {
    pub fn variant<'cx>(&self, model: &'cx Model) -> View<'cx, VariantNode> {
        model.get_variant(self.variant_index)
    }

    pub fn field<'cx>(&self, model: &'cx Model) -> View<'cx, FieldNode> {
        model.get_field(self.field_index)
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
#[derive(Debug, Clone)]
pub struct Pattern {
    entitlements: IndexMap<FieldIndex, Entitlement>,
}

impl Pattern {
    /// Add a new entitlement to the pattern. If an entitlement from the same field already exists,
    /// returns [`PatternError::FieldOccupied`].
    pub fn push(&mut self, entitlement: Entitlement) -> Result<(), PatternError> {
        if let Some(existing) = self.entitlements.get(entitlement.field_index()) {
            Err(PatternError::FieldOccupied(*existing))
        } else {
            self.entitlements
                .insert(entitlement.field_index, entitlement);

            Ok(())
        }
    }

    /// Determines whether this pattern satisfies another or not, recursively traversing the statewise and ontological
    /// entitlement spaces of entitlements involved if need be.
    ///
    /// *Note: This method is symmetrical in the sense that if "self" and "other" are swapped the outcome will be the
    /// same.*
    pub fn satisfies_pattern(&self, model: &Model, other: &Self) -> bool {
        let mut merge = IndexMap::<FieldIndex, IndexSet<VariantIndex>>::new();

        for pattern in [self, other] {
            for entitlement in pattern.entitlements.values() {
                merge
                    .entry(entitlement.field_index)
                    .or_default()
                    .insert(entitlement.variant_index);
            }
        }

        if merge.values().all(|bucket| bucket.len() == 1) {
            // they do satisfy each other, unless:

            for lhs in self.entitlements.values() {
                for rhs in other.entitlements.values() {
                    // if statewise spaces between any two opposing states
                    // OR
                    // the ontological spaces between any two opposing fields
                    // are seperable, then the the patterns do not satisfy each other

                    // skip if the states compared are identical
                    if lhs == rhs {
                        continue;
                    }

                    // optimization: if the state being compared against is present in this pattern (or vice versa), we
                    // already know the statewise entitlement spaces are inseperable, otherwise the pattern would be
                    // invalid

                    if let Some(existing) = self.entitlements.get(rhs.field_index())
                        && existing.variant_index() == rhs.variant_index()
                    {
                        continue;
                    }

                    if let Some(existing) = other.entitlements.get(lhs.field_index())
                        && existing.variant_index() == lhs.variant_index()
                    {
                        continue;
                    }

                    if let (Some(lhs_space), Some(rhs_space)) = (
                        lhs.variant(model).statewise_entitlements(),
                        rhs.variant(model).statewise_entitlements(),
                    ) && lhs_space.is_seperable_against(&rhs_space)
                    {
                        return false;
                    }

                    // optimization: if the field being compared against is present in this pattern, we already know
                    // the ontological entitlement spaces are inseperable, otherwise the pattern would be invalid

                    if self.entitlements.contains_key(rhs.field_index())
                        || other.entitlements.contains_key(lhs.field_index())
                    {
                        continue;
                    }

                    if let (Some(lhs_space), Some(rhs_space)) = (
                        lhs.field(model).ontological_entitlements(),
                        rhs.field(model).ontological_entitlements(),
                    ) && lhs_space.is_seperable_against(&rhs_space)
                    {
                        return false;
                    }
                }
            }

            true
        } else {
            false
        }
    }

    /// Determines whether this pattern satisfies the provided space or not.
    pub fn satisfies_space(&self, model: &Model, space: &Space) -> bool {
        space
            .patterns
            .iter()
            .any(|pattern| self.satisfies_pattern(model, pattern))
    }
}

pub enum PatternError {
    FieldOccupied(Entitlement),
}

/// A space is a set of [`Pattern`]s. A space is considered *satisfied* if **any** of its patterns are satisfied.
///
/// *Note: If a pattern in the space is a superset of another pattern in the space, it is extraneous.*
#[derive(Debug, Clone, Default)]
pub struct Space {
    patterns: Vec<Pattern>,
}

impl Space {
    /// Add a new pattern to the space.
    pub fn push(&mut self, pattern: Pattern) {
        self.patterns.push(pattern);
    }
}

impl<'cx> View<'cx, Space> {
    /// Determine whether the spaces are seperable or not.
    ///
    /// Spaces are seperable if for any combination of states, at most one space is satisfied.
    pub fn is_seperable_against(&self, other: &Space) -> bool {
        for lhs in &self.patterns {
            for rhs in &other.patterns {
                if lhs.satisfies_pattern(self.model, rhs) {
                    return false;
                }
            }
        }

        true
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
