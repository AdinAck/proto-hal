use model::field::numericity::Numericity;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

/// The type used for a value read from a field.
pub fn read_value_ty(
    peripheral_path: &Path,
    register_ident: &Ident,
    field_ident: &Ident,
    read_numericity: &Numericity,
) -> TokenStream {
    match read_numericity {
        Numericity::Numeric(..) => quote! { u32 },
        Numericity::Enumerated(..) => quote! {
            #peripheral_path::#register_ident::#field_ident::ReadVariant
        },
    }
}
