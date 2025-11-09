use ir::structures::field::Field;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

pub fn input_ty(
    register_path: &Path,
    field_ident: &Ident,
    field: &Field,
    input_generic: Option<&Ident>,
) -> TokenStream {
    let ty_name = field.type_name();

    if let Some(generic) = input_generic {
        quote! { #register_path::#field_ident::#ty_name<#generic> }
    } else {
        quote! { #register_path::#field_ident::#ty_name<::proto_hal::stasis::Dynamic> }
    }
}
