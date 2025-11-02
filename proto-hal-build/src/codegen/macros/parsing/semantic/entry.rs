use ir::structures::field::Field;
use syn::Ident;

use crate::codegen::macros::{
    diagnostic::Diagnostic,
    parsing::{
        semantic::Transition,
        syntax::{self, Binding},
    },
};

/// The semantic entry optionally provided at the termination of a path.
pub enum Entry<'cx> {
    /// There is no entry.
    Empty,
    /// The entry is a view binding with no transition.
    ///
    /// ```ignore
    /// (&foo)
    /// ```
    View(&'cx Binding),
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
}

impl<'cx> Entry<'cx> {
    /// Parse the entry input against the model to produce a semantic entry.
    pub fn parse(
        entry: &'cx syntax::Entry,
        field: &'cx Field,
        field_ident: &'cx Ident,
    ) -> Result<Self, Diagnostic> {
        Ok(match (&entry.binding, &entry.transition) {
            (None, None) => Self::Empty,
            (None, Some(transition)) => {
                Self::UnboundDynamicTransition(Transition::parse(transition, field, field_ident)?)
            }
            (Some(binding), None) if binding.is_viewed() => Self::View(binding),
            (Some(binding), None) => Err(Diagnostic::binding_must_be_view(binding))?,
            (Some(binding), Some(transition)) if binding.is_dynamic() => {
                Self::BoundDynamicTransition(
                    binding,
                    Transition::parse(transition, field, field_ident)?,
                )
            }
            (Some(binding), Some(transition)) if binding.is_moved() => {
                Self::StaticTransition(binding, Transition::parse(transition, field, field_ident)?)
            }
            (Some(binding), Some(..)) => Err(Diagnostic::binding_cannot_be_view(binding))?,
        })
    }
}
