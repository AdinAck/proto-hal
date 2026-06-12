use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Path;

use crate::macros::{gates::fragments::write_argument_value, parsing::semantic};

pub fn modify_argument_value(
    peripheral_path: &Path,
    register_path: &Path,
    field_path: &Path,
    field: &FieldNode,
    transition: &semantic::Transition,
    closure_args: Option<&TokenStream>,
) -> TokenStream {
    let write_expr = write_argument_value(
        peripheral_path,
        register_path,
        field_path,
        field,
        transition,
    );

    quote! { |#closure_args| #write_expr }
}
