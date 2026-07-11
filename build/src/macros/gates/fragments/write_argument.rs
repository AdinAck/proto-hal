use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Path, spanned::Spanned as _};

use crate::macros::{
    gates::fragments::write_argument_value, parsing::semantic::policies::field::GateEntry,
};

pub fn write_argument<'cx>(
    peripheral_path: &Path,
    register_path: &Path,
    field_path: &Path,
    field: &FieldNode,
    entry: &GateEntry<'cx>,
) -> TokenStream {
    match entry {
        GateEntry::View(binding) | GateEntry::Dynamic(binding) | GateEntry::Static(binding, ..) => {
            binding.to_token_stream()
        }
        GateEntry::DynamicTransition(binding, transition) => {
            let binding = binding.as_ref();
            let value = write_argument_value(
                peripheral_path,
                register_path,
                field_path,
                field,
                transition,
            );
            let span = field_path.span();

            quote_spanned! { span => (#binding, #value) }
        }
    }
}
