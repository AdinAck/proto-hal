use model::{Model, field::FieldNode};
use syn::Ident;

use crate::macros::{
    diagnostic::Diagnostic,
    parsing::{
        semantic::Transition,
        syntax::{self, Binding},
    },
};

/// The semantic entry optionally provided at the termination of a path to a field.
pub enum FieldEntry<'cx> {
    /// There is no entry.
    Empty,
    /// The entry is a view binding with no transition.
    ///
    /// ```ignore
    /// (&foo)
    /// ```
    View(&'cx Binding),
    /// The entry is a volatile view binding with no transition.
    ///
    /// ```ignore
    /// (&mut foo)
    /// ```
    BoundDynamic(&'cx Binding),
    /// The entry is a dyamic binding and a transition.
    ///
    /// ```ignore
    /// (&mut foo) => bar
    /// ```
    BoundDynamicTransition(&'cx Binding, Transition<'cx>),
    /// The entry is only a transition.
    ///
    /// ```ignore
    /// => bar
    /// ```
    UnboundDynamicTransition(Transition<'cx>),
    /// The entry is a static binding and a transition.
    ///
    /// ```ignore
    /// (foo) => bar
    /// ```
    StaticTransition(&'cx Binding, Transition<'cx>),
    /// The entry is just a static binding.
    ///
    /// ```ignore
    /// (foo)
    /// ```
    Consumed(&'cx Binding),
}

/// The semantic entry optionally provided at the termination of a path to a peripheral.
pub enum PeripheralEntry<'cx> {
    /// The entry is just a static binding.
    ///
    /// ```ignore
    /// (foo)
    /// ```
    Consumed(&'cx Binding),
}

impl<'cx> FieldEntry<'cx> {
    /// Parse the entry input against the model to produce a semantic entry.
    pub fn parse(
        model: &'cx Model,
        entry: &'cx syntax::Entry,
        field: &'cx FieldNode,
        field_ident: &'cx Ident,
    ) -> Result<Self, Diagnostic> {
        Ok(match (&entry.binding, &entry.transition) {
            (None, None) => Self::Empty,
            (None, Some(transition)) => Self::UnboundDynamicTransition(Transition::parse(
                model,
                transition,
                field,
                field_ident,
            )?),
            (Some(binding), None) if binding.is_viewed() => Self::View(binding),
            (Some(binding), None) if binding.is_dynamic() => Self::BoundDynamic(binding),
            (Some(binding), None) => Self::Consumed(binding),
            (Some(binding), Some(transition)) if binding.is_dynamic() => {
                Self::BoundDynamicTransition(
                    binding,
                    Transition::parse(model, transition, field, field_ident)?,
                )
            }
            (Some(binding), Some(transition)) if binding.is_moved() => Self::StaticTransition(
                binding,
                Transition::parse(model, transition, field, field_ident)?,
            ),
            (Some(binding), Some(..)) => Err(Diagnostic::binding_cannot_be_view(binding))?,
        })
    }
}

impl<'cx> PeripheralEntry<'cx> {
    /// Parse the entry input against the model to produce a semantic entry.
    pub fn parse(
        entry: &'cx syntax::Entry,
        peripheral_ident: &'cx Ident,
    ) -> Result<Self, Diagnostic> {
        Ok(match (&entry.binding, &entry.transition) {
            (None, None) => Err(Diagnostic::expected_binding(peripheral_ident))?,
            (.., Some(transition)) => Err(Diagnostic::unexpected_transition(transition))?,
            (Some(binding), None) if binding.is_viewed() => {
                Err(Diagnostic::binding_cannot_be_view(binding))?
            }
            (Some(binding), None) => Self::Consumed(binding),
        })
    }
}
