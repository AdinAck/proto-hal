use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote_spanned};
use syn::{Ident, Path};

use crate::codegen::macros::{
    gates::fragments::modify_argument_value, parsing::semantic::policies::field::RequireBinding,
};

pub fn modify_argument<'cx>(
    register_path: &Path,
    field_ident: &Ident,
    field: &FieldNode,
    entry: &RequireBinding<'cx>,
    closure_arguments: Option<&TokenStream>,
) -> TokenStream {
    match entry {
        RequireBinding::View(binding)
        | RequireBinding::Dynamic(binding)
        | RequireBinding::Static(binding, ..) => binding.to_token_stream(),
        RequireBinding::DynamicTransition(binding, transition) => {
            let binding = binding.as_ref();
            let value = modify_argument_value(
                register_path,
                field_ident,
                field,
                transition,
                closure_arguments,
            );
            let span = field_ident.span();

            quote_spanned! { span => (#binding, #value) }
        }
    }
}
