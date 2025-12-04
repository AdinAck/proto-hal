use model::field::{FieldNode, numericity::Numericity};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{Ident, Path, spanned::Spanned as _};

use crate::macros::parsing::semantic;

pub fn write_argument_value(
    register_path: &Path,
    field_ident: &Ident,
    field: &FieldNode,
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

    if let Some(Numericity::Enumerated(..)) = field.access.get_write() {
        quote! {{
            use #register_path::#field_ident::WriteVariant::{self as Variant, *};
            #block
        }}
    } else {
        block
    }
}
