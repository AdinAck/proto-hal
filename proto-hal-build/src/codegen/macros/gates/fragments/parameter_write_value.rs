use ir::structures::field::{Field, Numericity};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::codegen::macros::parsing::semantic;

pub fn parameter_write_value(
    register_path: &Path,
    field_ident: &Ident,
    field: &Field,
    transition: &semantic::Transition,
) -> TokenStream {
    let block = match transition {
        semantic::Transition::Variant(transition, ..) => {
            quote! { #transition as u32 }
        }
        semantic::Transition::Expr(expr) => {
            quote! { #expr as u32 }
        }
        semantic::Transition::Lit(lit_int) => quote! { #lit_int },
    };

    if let Some(Numericity::Enumerated { .. }) =
        field.access.get_read().map(|read| &read.numericity)
    {
        quote! {{
            #[allow(unused_imports)]
            use #register_path::#field_ident::write::Variant::*;
            #block
        }}
    } else {
        block
    }
}
