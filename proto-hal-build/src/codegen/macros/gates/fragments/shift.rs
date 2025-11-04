use proc_macro2::TokenStream;
use quote::quote;

pub fn shift(offset: u8) -> Option<TokenStream> {
    (offset != 0).then_some(quote! { << #offset })
}
