use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::codegen::macros::parsing::semantic;

pub fn write_value(
    register_path: &Path,
    field_ident: &Ident,
    transition: &semantic::Transition,
) -> TokenStream {
    match transition {
        semantic::Transition::Variant(transition, ..) => {
            quote! {{
                #[allow(unused_imports)]
                use #register_path::#field_ident::write::Variant::*;
                #transition as u32
            }}
        }
        semantic::Transition::Expr(expr) => {
            quote! {{
                #expr as u32
            }}
        }
        semantic::Transition::Lit(lit_int) => quote! { #lit_int },
    }
}
