use model::field::numericity::Numericity;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Path;

pub fn write_value_ty(
    peripheral_path: &Path,
    register_path: &Path,
    field_path: &Path,
    write_numericity: &Numericity,
) -> TokenStream {
    match write_numericity {
        Numericity::Numeric(..) => quote! { u32 },
        Numericity::Enumerated(..) => quote! {
            #peripheral_path::#register_path::#field_path::WriteVariant
        },
    }
}
