//! Policies employed by semantic parsing structures for altering the
//! parsing behavior.

use derive_more::Deref;

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{Entry, FieldEntryRefinementInput, Transition},
        syntax::Binding,
    },
};

/// This policy dictates what kind of items are permitted to be
/// parsed or not.
pub trait Filter {
    /// Whether the item class is permitted or not.
    fn accepted() -> bool;
}

/// This policy dictates how to refine the flat semantic input
/// into different refinement types.
pub trait Refine<'item>: Sized {
    /// The input context to be refined.
    type Input;

    /// Refine the input context into the refinement type, or
    /// fail trying.
    fn refine(item: Self::Input) -> Result<Self, Diagnostics>;
}

/// Forbid peripheral items from being parsed.
pub struct ForbidPeripherals;

impl Filter for ForbidPeripherals {
    fn accepted() -> bool {
        false
    }
}

/// Permit peripheral items to be parsed.
pub struct PermitPeripherals;

impl Filter for PermitPeripherals {
    fn accepted() -> bool {
        true
    }
}

/// Entries are forbidden from being specified.
pub struct ForbidEntry;

impl<'cx> Refine<'cx> for ForbidEntry {
    type Input = FieldEntryRefinementInput<'cx>;

    fn refine((.., entry): Self::Input) -> Result<Self, Diagnostics> {
        match entry {
            Entry::Empty => Ok(Self),
            Entry::View(binding) => Err(vec![Diagnostic::unexpected_binding(binding)]),
            Entry::BoundDynamicTransition(binding, transition)
            | Entry::StaticTransition(binding, transition) => Err(vec![
                Diagnostic::unexpected_binding(binding),
                Diagnostic::unexpected_transition(&transition),
            ]),
            Entry::UnboundDynamicTransition(transition) => {
                Err(vec![Diagnostic::unexpected_transition(&transition)])
            }
        }
    }
}

/// Only the transition component of the entry may be specified.
#[derive(Deref)]
pub struct PermitTransition<'cx>(Option<Transition<'cx>>);

impl<'cx> Refine<'cx> for PermitTransition<'cx> {
    type Input = FieldEntryRefinementInput<'cx>;

    fn refine((.., entry): Self::Input) -> Result<Self, Diagnostics> {
        match entry {
            Entry::Empty => Ok(Self(None)),
            Entry::View(binding) => Err(vec![Diagnostic::unexpected_binding(binding)]),
            Entry::BoundDynamicTransition(binding, ..) | Entry::StaticTransition(binding, ..) => {
                Err(vec![Diagnostic::unexpected_binding(binding)])
            }
            Entry::UnboundDynamicTransition(transition) => Ok(Self(Some(transition))),
        }
    }
}

/// The entry solely consists of a transition.
#[derive(Deref)]
pub struct TransitionOnly<'cx>(Transition<'cx>);

impl<'cx> Refine<'cx> for TransitionOnly<'cx> {
    type Input = FieldEntryRefinementInput<'cx>;

    fn refine((ident, entry): Self::Input) -> Result<Self, Diagnostics> {
        Ok(match entry {
            Entry::Empty => Err(Diagnostic::expected_transition(ident))?,
            Entry::View(binding) => Err(Diagnostic::unexpected_binding(binding))?,
            Entry::BoundDynamicTransition(binding, ..) | Entry::StaticTransition(binding, ..) => {
                Err(Diagnostic::unexpected_binding(binding))?
            }
            Entry::UnboundDynamicTransition(transition) => Self(transition),
        })
    }
}

/// The binding component of the entry must be specified.
pub enum RequireBinding<'cx> {
    /// The entry is a view (see [`Entry`]).
    View(&'cx Binding),
    /// The entry is dynnamic (see [`Entry`]).
    Dynamic(&'cx Binding, Transition<'cx>),
    /// The entry is static (see [`Entry`]).
    Static(&'cx Binding, Transition<'cx>),
}

impl<'cx> Refine<'cx> for RequireBinding<'cx> {
    type Input = FieldEntryRefinementInput<'cx>;

    fn refine((ident, entry): Self::Input) -> Result<Self, Diagnostics> {
        match entry {
            Entry::Empty => Err(vec![Diagnostic::expected_binding(ident)]),
            Entry::View(binding) => Ok(Self::View(binding)),
            Entry::BoundDynamicTransition(binding, transition) => {
                Ok(Self::Dynamic(binding, transition))
            }
            Entry::StaticTransition(binding, transition) => Ok(Self::Static(binding, transition)),
            Entry::UnboundDynamicTransition(..) => Err(vec![Diagnostic::expected_binding(ident)]),
        }
    }
}
