use std::ops::Deref;

use syn::Ident;

use crate::codegen::macros::{diagnostic::Diagnostic, parsing::syntax::Binding};

pub trait Policy<'item>: Sized {
    type Item;

    fn derive(item: &Self::Item) -> Result<Self, Diagnostic>;
}

/// Entries are required to NOT contain a binding.
pub struct NoBinding;

impl<'item> Policy<'item> for NoBinding {
    type Item = (&'item Ident, Option<&'item Binding>);

    fn derive(item: &Self::Item) -> Result<Self, Diagnostic> {
        if let Some(binding) = item.1 {
            Err(Diagnostic::unexpected_binding(binding))
        } else {
            Ok(Self)
        }
    }
}

/// Entries are required to contain a binding.
pub struct WithBinding<'args>(&'args Binding);

impl<'args> Policy<'args> for WithBinding<'args> {
    type Item = (&'args Ident, Option<&'args Binding>);

    fn derive(item: &Self::Item) -> Result<Self, Diagnostic> {
        if let Some(binding) = item.1 {
            Ok(Self(binding))
        } else {
            Err(Diagnostic::expected_binding(item.0))
        }
    }
}

impl<'args> Deref for WithBinding<'args> {
    type Target = Binding;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
