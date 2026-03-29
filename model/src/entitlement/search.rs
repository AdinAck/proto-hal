use derive_more::{AsMut, AsRef, Deref, DerefMut};
use indexmap::{IndexMap, IndexSet};

use crate::{
    Entitlement, Model,
    entitlement::{Axis, Pattern, Space, pattern},
    field::FieldIndex,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deref, DerefMut, AsRef, AsMut)]
struct Combination(Pattern);

impl Combination {
    fn insert(&mut self, field: FieldIndex, entitlement: Entitlement) {
        self.0
            .entitlements
            .insert(field, IndexSet::from([entitlement]));
    }
}

pub struct Search<'cx> {
    model: &'cx Model,
    /// The patterns which have already been traversed for validation.
    seen: Vec<Pattern>,
}

impl<'cx> Search<'cx> {
    pub fn new(model: &'cx Model) -> Self {
        Self {
            model,
            seen: Default::default(),
        }
    }

    pub fn validate_pattern(&mut self, pattern: &Pattern) -> Result<(), pattern::Error> {
        if self.seen.contains(pattern) {
            return Ok(());
        }

        self.seen.push(pattern.clone());

        if self.pattern_contradicts_pattern(pattern, &Pattern::tautology()) {
            // the only pattern which contradicts the tautology is the contradiction
            // which is not a valid pattern as a *valid* pattern is defined to be
            // *satisfiable*
            Err(pattern::Error::StructuralContradiction)?
        }

        if let Some((axis, contradictory_space)) = pattern
            .fields(self.model)
            .filter_map(|field| Some((Axis::Ontological, field.ontological_entitlements()?)))
            .chain(pattern.entitlements().filter_map(|entitlement| {
                Some((
                    Axis::Statewise,
                    entitlement.variant(self.model).statewise_entitlements()?,
                ))
            }))
            .find(|(.., space)| self.pattern_contradicts_space(pattern, space))
        {
            // this pattern cannot be satisfied because the only satisfying state
            // combinations are impossible due to entitlement constraints
            Err(pattern::Error::Contradicts {
                pattern: pattern.clone(),
                space: (*contradictory_space).clone(),
                axis,
            })?
        }

        Ok(())
    }

    pub fn pattern_contradicts_space(&mut self, pattern: &Pattern, space: &Space) -> bool {
        space
            .patterns
            .iter()
            .all(|other| self.pattern_contradicts_pattern(pattern, other))
    }

    pub fn pattern_contradicts_pattern(&mut self, lhs: &Pattern, rhs: &Pattern) -> bool {
        let intersection = lhs.intersection_with(rhs);

        if !intersection.entitlements.values().any(|s| s.is_empty()) {
            // if the patterns don't trivially contradict, deep search

            for combination in Self::combinations(&intersection.entitlements) {
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

            entitlements
                .get(field_index)
                .expect("field index must exist")
                .iter()
                .flat_map(|entitlement| {
                    children.iter().cloned().map(|mut child| {
                        child.insert(*field_index, *entitlement);

                        child
                    })
                })
                .collect()
        }

        inner(entitlements, entitlements.keys())
    }
}
