use model::field::FieldNode;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::macros::{gates::fragments::write_argument_value, parsing::semantic};

pub fn modify_argument_value(
    peripheral_path: &Path,
    register_ident: &Ident,
    field_ident: &Ident,
    field: &FieldNode,
    transition: &semantic::Transition,
    closure_args: Option<&TokenStream>,
) -> TokenStream {
    let write_expr = write_argument_value(
        peripheral_path,
        register_ident,
        field_ident,
        field,
        transition,
    );

    quote! { |#closure_args| #write_expr }
}
