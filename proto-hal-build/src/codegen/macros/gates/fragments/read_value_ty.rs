use ir::structures::field::numericity::Numericity;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

/// The type used for a value read from a field.
pub fn read_value_ty(
    register_path: &Path,
    field_ident: &Ident,
    read_numericity: &Numericity,
) -> TokenStream {
    match read_numericity {
        Numericity::Numeric(..) => quote! { u32 },
        Numericity::Enumerated(..) => quote! {
            #register_path::#field_ident::ReadVariant
        },
    }
}
