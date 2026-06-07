use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Path, spanned::Spanned as _};

use crate::macros::{
    gates::fragments::modify_argument_value, parsing::semantic::policies::field::GateEntry,
};

pub fn modify_argument<'cx>(
    peripheral_path: &Path,
    register_path: &Path,
    field_path: &Path,
    field: &FieldNode,
    entry: &GateEntry<'cx>,
    closure_arguments: Option<&TokenStream>,
) -> TokenStream {
    match entry {
        GateEntry::View(binding) | GateEntry::Dynamic(binding) | GateEntry::Static(binding, ..) => {
            binding.to_token_stream()
        }
        GateEntry::DynamicTransition(binding, transition) => {
            let binding = binding.as_ref();
            let value = modify_argument_value(
                peripheral_path,
                register_path,
                field_path,
                field,
                transition,
                closure_arguments,
            );
            let span = field_path.span();

            quote_spanned! { span => (#binding, #value) }
        }
    }
}
