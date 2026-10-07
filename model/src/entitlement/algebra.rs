//! The algebra of [entitlement spaces](super#entitlement-spaces) in [normal form](self#normal-form).
//!
//! # Normal form
//!
//! A pattern is in *normal form* when the pattern lists the entitled states by field. A space is in normal form when
//! the patterns of the space are as well, and no pattern of the space is [extraneous](super#covering). *Normalizing* a
//! space produces the space in normal form, which is an infallible transformation.
//!
//! The algebra compares states by identity, which is exact, as entitlements denote [states](crate::field#states) by
//! value.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use crate::{Word, entitlement::Entitlement, field::FieldId};

/// A [space](super::Space) in [normal form](self#normal-form).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NormalSpace {
    patterns: BTreeSet<NormalPattern>,
}

/// A [pattern](super::Pattern) in [normal form](self#normal-form).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalPattern {
    states: BTreeMap<FieldId, BTreeSet<Word>>,
}

impl NormalSpace {
    /// Create a space in normal form from the provided patterns, omitting patterns [covered](super#covering) by
    /// another.
    pub(crate) fn new(patterns: impl IntoIterator<Item = NormalPattern>) -> Self {
        let mut space = Self::default();

        for pattern in patterns {
            space.patterns.retain(|existing| !pattern.covers(existing));

            if !space
                .patterns
                .iter()
                .any(|existing| existing.covers(&pattern))
            {
                space.patterns.insert(pattern);
            }
        }

        space
    }
}

impl NormalPattern {
    /// Create a pattern in normal form from the provided entitlements.
    pub(crate) fn new(entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        let mut pattern = Self::default();

        for entitlement in entitlements {
            pattern
                .states
                .entry(entitlement.field)
                .or_default()
                .insert(entitlement.state);
        }

        pattern
    }

    /// Intersect the pattern with the provided pattern, producing the pattern satisfied when both patterns are
    /// satisfied. If the patterns [trivially contradict](super#contradiction), returns `None`.
    pub(crate) fn intersection(&self, other: &Self) -> Option<Self> {
        let mut states = self.states.clone();

        for (&field, other_states) in &other.states {
            match states.entry(field) {
                Entry::Vacant(entry) => {
                    entry.insert(other_states.clone());
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().retain(|state| other_states.contains(state));

                    if entry.get().is_empty() {
                        None?
                    }
                }
            }
        }

        Some(Self { states })
    }

    /// Determine whether the pattern [covers](super#covering) the provided pattern or not.
    pub(crate) fn covers(&self, other: &Self) -> bool {
        self.states.iter().all(|(field, states)| {
            other
                .states
                .get(field)
                .is_some_and(|covered| states.is_superset(covered))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{NormalPattern, NormalSpace};
    use crate::{Word, entitlement::Entitlement, field::FieldId};

    #[test]
    fn intersection() {
        let [foo, bar, baz, qux] = [0, 1, 2, 3].map(FieldId::from);
        let [a, b, c] = [0, 1, 2];

        let pattern_0 = pattern([(foo, a), (bar, b), (bar, c)]);

        let cases = [
            (
                "`foo.0 & bar.{1, 2}` and `bar.0 & foo.0`",
                pattern_0.clone(),
                pattern([(bar, a), (foo, a)]),
                None,
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.0 & foo.1`",
                pattern_0.clone(),
                pattern([(bar, a), (foo, b)]),
                None,
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.1 & foo.0`",
                pattern_0.clone(),
                pattern([(bar, b), (foo, a)]),
                Some(pattern([(foo, a), (bar, b)])),
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.{1, 2} & foo.{0, 1}`",
                pattern_0.clone(),
                pattern([(bar, b), (bar, c), (foo, a), (foo, b)]),
                Some(pattern_0.clone()),
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.{0, 1} & foo.{0, 1}`",
                pattern_0.clone(),
                pattern([(bar, a), (bar, b), (foo, a), (foo, b)]),
                Some(pattern([(foo, a), (bar, b)])),
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.1`",
                pattern_0.clone(),
                pattern([(bar, b)]),
                Some(pattern([(foo, a), (bar, b)])),
            ),
            (
                "`foo.0 & bar.{1, 2}` and `bar.1 & baz.1`",
                pattern_0.clone(),
                pattern([(bar, b), (baz, b)]),
                Some(pattern([(foo, a), (bar, b), (baz, b)])),
            ),
            (
                "`qux.0` and `qux.1`",
                pattern([(qux, 0)]),
                pattern([(qux, 1)]),
                None,
            ),
            (
                "`qux.{0, 1}` and `qux.1`",
                pattern([(qux, 0), (qux, 1)]),
                pattern([(qux, 1)]),
                Some(pattern([(qux, 1)])),
            ),
        ];

        similar_asserts::assert_eq!(
            expected: cases
                .iter()
                .map(|(description, .., intersection)| (*description, intersection.clone()))
                .collect::<Vec<_>>(),
            found: cases
                .iter()
                .map(|(description, lhs, rhs, ..)| (*description, lhs.intersection(rhs)))
                .collect::<Vec<_>>(),
            "expected the intersection of each pair of patterns to list, for each field, the states both patterns \
             list, or to be `None` when the patterns trivially contradict",
        );
    }

    #[test]
    fn covering() {
        let [foo, bar, qux] = [0, 1, 2].map(FieldId::from);
        let [a, b] = [0, 1];

        let cases = [
            (
                "`foo.0` and `foo.0 & bar.1`",
                vec![pattern([(foo, a)]), pattern([(foo, a), (bar, b)])],
                space([pattern([(foo, a)])]),
            ),
            (
                "`foo.0 & bar.1` and `foo.0`",
                vec![pattern([(foo, a), (bar, b)]), pattern([(foo, a)])],
                space([pattern([(foo, a)])]),
            ),
            (
                "`foo.0` and `foo.{0, 1}`",
                vec![pattern([(foo, a)]), pattern([(foo, a), (foo, b)])],
                space([pattern([(foo, a), (foo, b)])]),
            ),
            (
                "`foo.0` and `bar.1`",
                vec![pattern([(foo, a)]), pattern([(bar, b)])],
                space([pattern([(foo, a)]), pattern([(bar, b)])]),
            ),
            (
                "`foo.0` and `foo.0`",
                vec![pattern([(foo, a)]), pattern([(foo, a)])],
                space([pattern([(foo, a)])]),
            ),
            (
                "`qux.0` and `qux.{0, 1}`",
                vec![pattern([(qux, 0)]), pattern([(qux, 0), (qux, 1)])],
                space([pattern([(qux, 0), (qux, 1)])]),
            ),
        ];

        similar_asserts::assert_eq!(
            expected: cases
                .iter()
                .map(|(description, .., space)| (*description, space.clone()))
                .collect::<Vec<_>>(),
            found: cases
                .iter()
                .map(|(description, patterns, ..)| {
                    (*description, NormalSpace::new(patterns.iter().cloned()))
                })
                .collect::<Vec<_>>(),
            "expected the space created from each list of patterns to omit the patterns covered by another",
        );
    }

    /// Create a pattern in normal form from entitlements of the provided fields to the states paired with them.
    fn pattern<const N: usize>(entitlements: [(FieldId, Word); N]) -> NormalPattern {
        NormalPattern::new(entitlements.map(|(field, state)| Entitlement { field, state }))
    }

    /// Create a space containing exactly the provided patterns.
    fn space<const N: usize>(patterns: [NormalPattern; N]) -> NormalSpace {
        NormalSpace {
            patterns: patterns.into(),
        }
    }
}
