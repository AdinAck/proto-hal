use ir::structures::field::{FieldNode, numericity::Numericity};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Path};

use crate::codegen::macros::parsing::{
    semantic::{self, policies::RequireBinding},
    syntax,
};

pub fn transition_return_ty<'cx>(
    register_path: &Path,
    entry: &RequireBinding,
    field: &FieldNode,
    field_ident: &Ident,
    output_generic: Option<&Ident>,
) -> Option<TokenStream> {
    let ty_name = field.type_name();

    if let Some(output_generic) = output_generic {
        return Some(quote! {
            #register_path::#field_ident::#ty_name<#output_generic>
        });
    }

    let numeric_ty = match field.access.get_write()? {
        Numericity::Numeric(numeric) => Some(numeric.ty(field.width).1),
        _ => None,
    };

    Some(match entry {
        RequireBinding::View(..) | RequireBinding::Dynamic(..) => None?,
        RequireBinding::Static(.., transition) => {
            match transition {
                semantic::Transition::Variant(transition, variant) => {
                    // note: this is done to preserve span when possible
                    let ty = match transition {
                        syntax::Transition::Expr(expr) => expr.to_token_stream(),
                        syntax::Transition::Lit(..) => variant.type_name().to_token_stream(),
                    };
                    quote! {
                        #register_path::#field_ident::#ty_name<#register_path::#field_ident::#ty>
                    }
                }
                semantic::Transition::Expr(expr) => {
                    // if the field is numeric, treat expr as a const generic
                    // otherwise, treat it as an (incomplete) variant.
                    // once the variant is complete, this will be handled
                    // in the `Variant` arm
                    let state = if let Some(numeric_ty) = numeric_ty {
                        quote! { ::proto_hal::stasis::#numeric_ty<#expr> }
                    } else {
                        quote! { #register_path::#field_ident::#expr }
                    };

                    quote! { #register_path::#field_ident::#ty_name<#state> }
                }
                semantic::Transition::Lit(lit_int) => {
                    let state = if let Some(numeric_ty) = numeric_ty {
                        quote! { ::proto_hal::stasis::#numeric_ty<#lit_int> }
                    } else {
                        // TODO: i don't understand this. why just dump the literal
                        // as a container generic?
                        quote! { #lit_int }
                    };

                    quote! { #register_path::#field_ident::#ty_name<#state> }
                }
            }
        }
    })
}
