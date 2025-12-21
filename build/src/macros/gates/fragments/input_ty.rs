use model::field::Field;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

pub fn input_ty(
    peripheral_path: &Path,
    register_ident: &Ident,
    field_ident: &Ident,
    field: &Field,
    input_generic: Option<&Ident>,
) -> TokenStream {
    let ty_name = field.type_name();

    if let Some(generic) = input_generic {
        quote! { #peripheral_path::#register_ident::#field_ident::#ty_name<#generic> }
    } else {
        quote! { #peripheral_path::#register_ident::#field_ident::#ty_name<::proto_hal::stasis::Dynamic> }
    }
}
