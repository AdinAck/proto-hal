use proc_macro2::TokenStream;
use quote::quote;

pub fn conjure() -> TokenStream {
    quote! { ::proto_hal::stasis::Conjure::conjure() }
}
