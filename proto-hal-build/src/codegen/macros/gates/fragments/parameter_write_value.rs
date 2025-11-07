use ir::structures::field::{Field, Numericity};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{Ident, Path, spanned::Spanned as _};

use crate::codegen::macros::parsing::semantic;

pub fn parameter_write_value(
    register_path: &Path,
    field_ident: &Ident,
    field: &Field,
    transition: &semantic::Transition,
) -> TokenStream {
    let block = match transition {
        semantic::Transition::Variant(transition, variant) => {
            let mut ident = variant.type_name();
            ident.set_span(transition.span());
            ident.to_token_stream()
        }
        semantic::Transition::Expr(expr) => expr.to_token_stream(),
        semantic::Transition::Lit(lit_int) => quote! { #lit_int },
    };

    if let Some(Numericity::Enumerated { .. }) =
        field.access.get_read().map(|read| &read.numericity)
    {
        quote! {{
            #[allow(unused_imports)]
            use #register_path::#field_ident::write::Variant::{self, *};
            #block
        }}
    } else {
        block
    }
}
