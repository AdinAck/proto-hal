use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Ident, Path};

use crate::macros::{
    gates::fragments::write_argument_value, parsing::semantic::policies::field::RequireBinding,
};

pub fn write_argument<'cx>(
    peripheral_path: &Path,
    register_ident: &Ident,
    field_ident: &Ident,
    field: &FieldNode,
    entry: &RequireBinding<'cx>,
) -> TokenStream {
    match entry {
        RequireBinding::View(binding)
        | RequireBinding::Dynamic(binding)
        | RequireBinding::Static(binding, ..)
        | RequireBinding::Consumed(binding) => binding.to_token_stream(),
        RequireBinding::DynamicTransition(binding, transition) => {
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
