//! Policies defining refinement of peripheral path/entry parsing.

use derive_more::{AsRef, Deref};
use syn::spanned::Spanned;

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    parsing::{
        semantic::{entry::PeripheralEntry, policies::Refine},
        syntax,
    },
};

/// Peripheral paths are forbidden from being specified.
pub struct ForbidPath;

impl<'cx> Refine<'cx> for ForbidPath {
    type Input = PeripheralEntry<'cx>;

    fn refine(cx: &impl Spanned, _input: Self::Input) -> Result<Self, Diagnostics> {
        Err(Diagnostic::unexpected_peripheral(cx))?
    }
}

/// Entries can only be "consume" bindings.
#[derive(Deref, AsRef)]
pub struct ConsumeOnly<'cx>(#[deref] &'cx syntax::Binding);

impl<'cx> Refine<'cx> for ConsumeOnly<'cx> {
    type Input = PeripheralEntry<'cx>;

    fn refine(_cx: &impl Spanned, entry: Self::Input) -> Result<Self, Diagnostics> {
        Ok(Self(match entry {
            PeripheralEntry::Consumed(binding) => binding,
        }))
    }
}
