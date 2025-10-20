use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

pub fn scaffolding<S>(reexports: impl IntoIterator<Item = S>) -> TokenStream
where
    S: AsRef<str>,
{
    let idents = reexports
        .into_iter()
        .map(|s| Ident::new(s.as_ref(), Span::call_site()));

    quote! {
        include!(concat!(env!("OUT_DIR"), "/hal.rs"));
        pub use macros::{#(#idents,)*};
    }
}
