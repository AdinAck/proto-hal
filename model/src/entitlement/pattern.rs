use indexmap::{IndexMap, IndexSet};

use crate::{
    Entitlement, Model,
    entitlement::{Axis, Space, search::Search},
    field::{
        FieldIndex, FieldNode,
        numericity::{Enumerated, Numericity},
    },
    model::View,
};

/// A set of [`Entitlement`]s, grouped by field.
///
/// A pattern is **satisfied** when *all* fields contain a satisfied entitlement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pattern {
    pub(super) entitlements: IndexMap<FieldIndex, IndexSet<Entitlement>>,
}

impl Pattern {
    /// Create a pattern with the provided entitlements.
    pub fn new(
        model: &Model,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) -> Result<Self, Error> {
        let pattern = Pattern::new_unchecked(model, entitlements);

        pattern.validate(model)?;

        Ok(pattern)
    }

    /// Create an *unvalidated* pattern with the provided entitlements.
    pub(super) fn new_unchecked(
        model: &Model,
        entitlements: impl IntoIterator<Item = Entitlement>,
    ) -> Self {
        let mut entitlement_map = IndexMap::<FieldIndex, IndexSet<Entitlement>>::new();

        for entitlement in entitlements {
            entitlement_map
                .entry(entitlement.field(model).index)
                .or_default()
                .insert(entitlement);
        }

        Self {
            entitlements: entitlement_map,
        }
    }

    /// Create a pattern that is always satisfied. The simplest being the empty
    /// pattern.
    pub fn tautology() -> Self {
        Self {
            entitlements: Default::default(),
        }
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
    pub fn validate(&self, model: &Model) -> Result<(), Error> {
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

    /// Produce the complement [`Space`] for this pattern.
    ///
    /// ## Meaning
    /// The complement of a pattern is a space where the space is satisfied iff this
    /// pattern is **not** satisfied.
    ///
    /// ## Definition
    /// The complement space of a pattern contains one pattern per field of the
    /// source pattern, where each pattern contains the complement of the respective
    /// field.
    ///
    /// The complement of a field in a pattern is the difference between the set of
    /// all possible entitlements of the field, and the specified ones.
    pub fn complement(&self, model: &Model) -> Space {
        Space::new(
            self.entitlements
                .iter()
                .filter_map(|(&field_index, entitlements)| {
                    let field = model.get_field(field_index);

                    // note: if a field is mentioned in a pattern it must be enumerated
                    // note: the choice of the read numericity vs write numericity is
                    //       arbitrary since resolvable fields are symmetrical
                    let Numericity::Enumerated(Enumerated { variants }) =
                        field.access.get_read()?
                    else {
                        None?
                    };

                    let whole = IndexSet::<Entitlement>::from_iter(
                        variants.values().copied().map(Entitlement),
                    );

                    let mut complement = whole.difference(entitlements).copied().peekable();

                    if complement.peek().is_none() {
                        // if the complement is empty the resulting pattern should be
                        // a contradiction, discard
                        None?
                    } else {
                        Pattern::new(model, complement).ok()
                    }
                }),
        )
    }

    /// The entitlements in this pattern.
    pub fn entitlements(&self) -> impl Iterator<Item = &Entitlement> {
        self.entitlements.values().flat_map(|field| field.iter())
    }

    pub fn fields<'cx>(&'cx self, model: &'cx Model) -> impl Iterator<Item = View<'cx, FieldNode>> {
        self.entitlements
            .keys()
            .map(|&field_index| model.get_field(field_index))
    }

    pub fn to_string(&self, model: &Model) -> String {
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
                    model.get_field(field_index).module_name(),
                    entitlement_idents
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(super) fn intersection_with(&self, other: &Self) -> Self {
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

        Self {
            entitlements: intersection,
        }
    }
}

/// The error that may be emitted when validating a pattern.
#[derive(Debug, Clone)]
pub enum Error {
    /// The pattern is a contradiction intrinsic to its structure and as such is
    /// unsatisfiable.
    StructuralContradiction,
    /// The pattern contradicts a space that causes it to be unsatisfiable.
    Contradicts {
        /// The invalid pattern.
        pattern: Pattern,
        /// The imposing space.
        space: Space,
        /// The axis of the imposing space.
        axis: Axis,
    },
}
