use crate::{
    Entitlement, Model,
    entitlement::{Pattern, pattern},
    field::FieldIndex,
};

/// A space is a set of [`Pattern`]s.
///
/// A space is **satisfied** when *any* of its [`Pattern`]s are satisfied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Space {
    pub(super) patterns: Vec<Pattern>,
}

impl Space {
    /// Create a space from the provided patterns.
    pub fn new(patterns: impl IntoIterator<Item = Pattern>) -> Self {
        let mut pattern_list = vec![];

        for pattern in patterns {
            pattern_list.retain(|existing| !pattern.covers(existing));

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
    ) -> Result<Self, pattern::Error> {
        Ok(Self::new(
            entitlements
                .into_iter()
                .map(|entitlements| Pattern::new(model, entitlements.into_iter()))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    /// Create a space that is never satisfied. The simplest being the empty space.
    pub fn contradiction() -> Self {
        Self {
            patterns: Default::default(),
        }
    }

    /// Determine whether the space contradicts the other or not.
    ///
    /// ## Meaning
    /// When a space contradicts another, it means that there **does not exist** a combination of states which
    /// satisfies both spaces.
    ///
    /// ## Definition
    /// A space contradicts another if **all** [`Pattern`]s in the space contradict the other space.
    pub fn contradicts(&self, model: &Model, other: &Space) -> bool {
        self.patterns
            .iter()
            .all(|pattern| pattern.contradicts_space(model, other))
    }

    /// Produce the complement [`Space`] for this space.
    ///
    /// ## Meaning
    /// The complement of a space is a space that is satisfied iff this space is
    /// **not** satisfied.
    ///
    /// *Note: If spaces A and B complement each other, then every pattern in A
    /// will contradict every pattern in B (and vice versa).*
    ///
    /// ## Definition
    /// The complement of a space is the cartesian of the complement spaces of each
    /// pattern in the space.
    ///
    /// The cartesian product of two spaces is the intersection of every pattern in
    /// each space and the other space.
    pub fn complement(&self, model: &Model) -> Self {
        self.patterns()
            .map(|pattern| pattern.complement(model))
            .reduce(|acc, next| {
                Space::new(acc.patterns().flat_map(|lhs| {
                    next.patterns().filter_map(|rhs| {
                        let intersection = lhs.intersection_with(rhs);
                        intersection.validate(model).ok()?;
                        Some(intersection)
                    })
                }))
            })
            .unwrap_or(Space::new([Pattern::tautology()]))
    }

    /// The [`Pattern`]s in the space.
    pub fn patterns(&self) -> impl Iterator<Item = &Pattern> + Clone {
        self.patterns.iter()
    }

    /// The [`FieldIndex`]s in the space.
    ///
    /// *Note: Field indicies may be produced more than once.*
    pub fn field_indicies(&self) -> impl Iterator<Item = &FieldIndex> {
        self.patterns()
            .flat_map(|pattern| pattern.entitlements.keys())
    }

    /// The [`Entitlement`]s in the space.
    pub fn entitlements(&self) -> impl Iterator<Item = &Entitlement> {
        self.patterns().flat_map(|pattern| pattern.entitlements())
    }

    /// The number of [`Pattern`]s in the space.
    pub fn count(&self) -> usize {
        self.patterns.len()
    }

    /// Determine whether the space is empty.
    ///
    /// *Note: This method will still return `true` if the space contains empty
    /// patterns.*
    pub fn is_empty(&self) -> bool {
        self.patterns()
            .next()
            .is_none_or(|p| p.entitlements.is_empty())
    }

    pub fn to_string(&self, model: &Model) -> String {
        let patterns = self
            .patterns()
            .map(|pattern| pattern.to_string(model))
            .collect::<Vec<_>>()
            .join(", ");

        format!("{{ {patterns} }}")
    }
}
