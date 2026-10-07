//! The model components related to [entitlements](Entitlement).
//!
//! # Entitlements
//!
//! An *entitlement* is a requirement for a particular field to inhabit a particular [state](crate::field#states). The
//! field is *implicated* by the entitlement, as the field houses the state entitled to. An entitlement denotes the
//! state entitled to by value, which identifies a variant if the implicated field is
//! [enumerated](crate::field#numericity).
//!
//! # Entitlement spaces
//!
//! A *pattern* is a set of entitlements, grouped by field. A pattern lists fields and their respective *entitled
//! states*, which are the states entitled to by the entitlements implicating the field. A pattern is *satisfied* when
//! *each* implicated field inhabits *any* one of its entitled states.
//!
//! An *entitlement space* is a set of patterns. A space is *satisfied* when any one pattern of the space is satisfied.
//!
//! A *tautology* is a space that is invariably satisfied, and a *contradiction* is a space that cannot be satisfied. A
//! space containing an empty pattern is the *trivial tautology*, and an empty space is the *trivial contradiction*.
//!
//! ## Contradiction
//!
//! A pattern *trivially contradicts* another when the patterns implicate a shared field with disjoint entitled states.
//!
//! Patterns *contradict* when the patterns cannot be satisfied simultaneously. A field inhabits a state only when the
//! field [exists](#ontological) and the [statewise](#statewise) entitlement space of the state is satisfied, so
//! patterns may contradict *without* trivially contradicting.
//!
//! ## Covering
//!
//! A pattern *covers* another when the set of fields the pattern implicates is a *subset* of the other's, and for each
//! field, the set of entitled states is a *superset* of the set for the corresponding field in the other pattern. A
//! pattern covered by another pattern of a space is *extraneous*, as removing the covered pattern does not affect the
//! satisfying conditions of the space.
//!
//! # Imposing entitlements
//!
//! Peripherals, fields, and variants *impose* entitlements in the form of [spaces](#entitlement-spaces), each kind of
//! space defining the requirements for a particular quality of the imposing item. An omitted space is considered a
//! tautology.
//!
//! ## Ontological
//!
//! An *ontological* entitlement space defines the requirements for the imposing peripheral or field to exist. The
//! item exists exactly when the space is satisfied.
//!
//! ## Affordance
//!
//! An *affordance* entitlement space defines the requirements for software to write to the imposing field.
//!
//! ## Hardware affordance
//!
//! A *hardware affordance* entitlement space defines the requirements for *hardware* to write to the imposing field.
//! Only fields with modality [`VolatileStore`](crate::field::access_modality::VolatileStore) may impose hardware
//! affordance entitlements, which determine whether such a field is [resolvable](crate::field#resolvability).
//!
//! ## Statewise
//!
//! A *statewise* entitlement space defines the requirements for a field to inhabit the imposing variant. Statewise
//! entitlements are imposed per field, so variants shared by fields that assume the same [schema](crate::schema)
//! may impose independent statewise entitlements.

pub(crate) mod algebra;

use source::Spanned;

use crate::{Word, field::FieldId};

/// A requirement for a particular field to inhabit a particular state. (*See [entitlements](self#entitlements)*)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Entitlement {
    /// The implicated field.
    pub field: FieldId,
    /// The [state](crate::field#states) the field is required to inhabit.
    pub state: Word,
}

/// A [pattern](self#entitlement-spaces) in the form it was written, listing entitlements in the order they were added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The entitlements, in the order they were added.
    pub entitlements: Vec<Spanned<Entitlement>>,
}

/// A set of [patterns](Pattern). (*See [entitlement spaces](self#entitlement-spaces)*)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// The patterns, in the order they were added.
    pub patterns: Vec<Spanned<Pattern>>,
}
