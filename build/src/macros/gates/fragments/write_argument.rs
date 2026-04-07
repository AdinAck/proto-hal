use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Ident, Path};

use crate::macros::{
    gates::fragments::write_argument_value, parsing::semantic::policies::field::GateEntry,
};

pub fn write_argument<'cx>(
    peripheral_path: &Path,
    register_ident: &Ident,
    field_ident: &Ident,
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
                register_ident,
                field_ident,
                field,
                transition,
            );
            let span = field_ident.span();

            quote_spanned! { span => (#binding, #value) }
        }
    }
}
