use ir::structures::field::Numericity;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

pub fn write_value_ty(
    register_path: &Path,
    field_ident: &Ident,
    write_numericity: &Numericity,
) -> TokenStream {
    match write_numericity {
        Numericity::Numeric => quote! { u32 },
        Numericity::Enumerated { .. } => quote! {
            #register_path::#field_ident::WriteVariant
        },
    }
}
