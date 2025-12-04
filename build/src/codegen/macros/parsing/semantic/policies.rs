//! Policies employed by semantic parsing structures for altering the parsing behavior.

pub mod field;
pub mod peripheral;

use syn::spanned::Spanned;

use crate::codegen::macros::diagnostic::Diagnostics;

/// This policy dictates how to refine the flat semantic input
/// into different refinement types.
pub trait Refine<'item>: Sized {
    /// The input type to be refined.
    type Input;

    /// Refine the input context into the refinement type, or
    /// fail trying.
    fn refine(cx: &impl Spanned, input: Self::Input) -> Result<Self, Diagnostics>;
}
