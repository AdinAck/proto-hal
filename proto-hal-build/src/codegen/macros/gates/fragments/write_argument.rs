use model::structures::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Ident, Path};

use crate::codegen::macros::{
    gates::fragments::write_argument_value, parsing::semantic::policies::field::RequireBinding,
};

pub fn write_argument<'cx>(
    register_path: &Path,
    field_ident: &Ident,
    field: &FieldNode,
    entry: &RequireBinding<'cx>,
) -> TokenStream {
    match entry {
        RequireBinding::View(binding) | RequireBinding::Static(binding, ..) => {
            binding.to_token_stream()
        }
        RequireBinding::Dynamic(binding, transition) => {
            let binding = binding.as_ref();
            let value = write_argument_value(register_path, field_ident, field, transition);
            let span = field_ident.span();

            quote_spanned! { span => (#binding, #value) }
        }
    }
}
