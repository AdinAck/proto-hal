//! Policies defining refinement of field path/entry parsing.

use derive_more::Deref;
use syn::spanned::Spanned;

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{self, FieldEntry, policies::Refine},
        syntax,
    },
};

/// Entries are forbidden from being specified.
pub struct ForbidEntry;

impl<'cx> Refine<'cx> for ForbidEntry {
    type Input = FieldEntry<'cx>;

    fn refine(_cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(match entry {
            FieldEntry::Empty => Self,
            FieldEntry::View(binding)
            | FieldEntry::BoundDynamic(binding)
            | FieldEntry::Consumed(binding) => Err(Diagnostic::unexpected_binding(binding))?,
            FieldEntry::BoundDynamicTransition(binding, transition)
            | FieldEntry::StaticTransition(binding, transition) => Err(vec![
                Diagnostic::unexpected_binding(binding),
                Diagnostic::unexpected_transition(&transition.span()),
            ])?,
            FieldEntry::UnboundDynamicTransition(transition) => {
                Err(Diagnostic::unexpected_transition(&transition.span()))?
            }
        })
    }
}

/// Only the transition component of the entry may be specified.
#[derive(Deref)]
pub struct PermitTransition<'cx>(Option<semantic::Transition<'cx>>);

impl<'cx> Refine<'cx> for PermitTransition<'cx> {
    type Input = FieldEntry<'cx>;

    fn refine(_cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(match entry {
            FieldEntry::Empty => Self(None),
            FieldEntry::View(binding)
            | FieldEntry::BoundDynamic(binding)
            | FieldEntry::Consumed(binding) => Err(Diagnostic::unexpected_binding(binding))?,
            FieldEntry::BoundDynamicTransition(binding, ..)
            | FieldEntry::StaticTransition(binding, ..) => {
                Err(Diagnostic::unexpected_binding(binding))?
            }
            FieldEntry::UnboundDynamicTransition(transition) => Self(Some(transition)),
        })
    }
}

/// The entry solely consists of a transition.
#[derive(Deref)]
pub struct TransitionOnly<'cx>(semantic::Transition<'cx>);

impl<'cx> Refine<'cx> for TransitionOnly<'cx> {
    type Input = FieldEntry<'cx>;

    fn refine(cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(match entry {
            FieldEntry::Empty => Err(Diagnostic::expected_transition(cx))?,
            FieldEntry::View(binding)
            | FieldEntry::BoundDynamic(binding)
            | FieldEntry::Consumed(binding) => Err(Diagnostic::unexpected_binding(binding))?,
            FieldEntry::BoundDynamicTransition(binding, ..)
            | FieldEntry::StaticTransition(binding, ..) => {
                Err(Diagnostic::unexpected_binding(binding))?
            }
            FieldEntry::UnboundDynamicTransition(transition) => Self(transition),
        })
    }
}

/// The binding component of the entry must be specified.
///
/// *Note: This policy does not accept "consume" bindings.*
pub enum RequireBinding<'cx> {
    /// The entry is a view (see [`Entry`]).
    View(&'cx syntax::Binding),
    /// The entry is dynnamic (see [`Entry`]).
    Dynamic(&'cx syntax::Binding),
    /// The entry is a dynnamic transition (see [`Entry`]).
    DynamicTransition(&'cx syntax::Binding, semantic::Transition<'cx>),
    /// The entry is static (see [`Entry`]).
    Static(&'cx syntax::Binding, semantic::Transition<'cx>),
}

impl<'cx> Refine<'cx> for RequireBinding<'cx> {
    type Input = FieldEntry<'cx>;

    fn refine(cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(match entry {
            FieldEntry::Empty => Err(Diagnostic::expected_binding(cx))?,
            FieldEntry::View(binding) => Self::View(binding),
            FieldEntry::BoundDynamic(binding) => Self::Dynamic(binding),
            FieldEntry::BoundDynamicTransition(binding, transition) => {
                Self::DynamicTransition(binding, transition)
            }
            FieldEntry::StaticTransition(binding, transition) => Self::Static(binding, transition),
            FieldEntry::Consumed(binding) => Err(Diagnostic::binding_cannot_be_consumed(binding))?,
            FieldEntry::UnboundDynamicTransition(..) => Err(Diagnostic::expected_binding(cx))?,
        })
    }
}

impl<'cx> RequireBinding<'cx> {
    /// View the binding component of the entry.
    pub fn binding(&self) -> &syntax::Binding {
        match self {
            RequireBinding::View(binding) => binding,
            RequireBinding::Dynamic(binding) => binding,
            RequireBinding::DynamicTransition(binding, ..) => binding,
            RequireBinding::Static(binding, ..) => binding,
        }
    }
}

/// The entry solely consists of a binding.
///
/// *Note: This policy does not accept "consume" bindings.*
#[derive(Deref)]
pub struct BindingOnly<'cx>(&'cx syntax::Binding);

impl<'cx> Refine<'cx> for BindingOnly<'cx> {
    type Input = FieldEntry<'cx>;

    fn refine(cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(Self(match entry {
            FieldEntry::Empty => Err(Diagnostic::expected_binding(cx))?,
            FieldEntry::View(binding) => binding,
            FieldEntry::BoundDynamic(binding) => binding,
            FieldEntry::Consumed(binding) => Err(Diagnostic::binding_cannot_be_consumed(binding))?,
            FieldEntry::BoundDynamicTransition(.., transition)
            | FieldEntry::UnboundDynamicTransition(transition)
            | FieldEntry::StaticTransition(.., transition) => {
                Err(Diagnostic::unexpected_transition(&transition.span()))?
            }
        }))
    }
}

/// Entries can only be "consume" bindings.
#[derive(Deref)]
pub struct ConsumeOnly<'cx>(#[deref] &'cx syntax::Binding);

impl<'cx> Refine<'cx> for ConsumeOnly<'cx> {
    type Input = FieldEntry<'cx>;

    fn refine(cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(Self(match entry {
            FieldEntry::Empty => Err(Diagnostic::expected_binding(cx))?,
            FieldEntry::View(binding) => Err(Diagnostic::binding_cannot_be_view(binding))?,
            FieldEntry::BoundDynamic(binding) => {
                Err(Diagnostic::binding_cannot_be_dynamic(binding))?
            }
            FieldEntry::BoundDynamicTransition(.., transition)
            | FieldEntry::UnboundDynamicTransition(transition)
            | FieldEntry::StaticTransition(.., transition) => {
                Err(Diagnostic::unexpected_transition(&transition.span()))?
            }
            FieldEntry::Consumed(binding) => binding,
        }))
    }
}
