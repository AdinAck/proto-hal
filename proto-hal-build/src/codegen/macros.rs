pub mod diagnostic;
mod gates;
pub mod parsing;
mod scaffolding;
mod unmask;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

pub use gates::{
    modify_untracked::modify_untracked,
    read::read,
    read_untracked::read_untracked,
    //     write::{write, write_in_place},
    write_untracked::{write_from_reset_untracked, write_from_zero_untracked},
};
pub use scaffolding::scaffolding;

pub fn reexports(args: TokenStream) -> TokenStream {
    let idents_raw = vec![
        "modify_untracked",
        "read",
        "read_untracked",
        // "write",
        // "write_in_place",
        "write_from_reset_untracked",
        "write_from_zero_untracked",
    ];

    let idents = idents_raw
        .iter()
        .map(|name| Ident::new(name, Span::call_site()))
        .collect::<Vec<_>>();

    quote! {
        #(
            #[proc_macro]
            pub fn #idents(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
                ::proto_hal_build::codegen::macros::#idents(&::model::generate(#args), tokens.into()).into()
            }
        )*

        #[proc_macro]
        pub fn scaffolding(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
            ::proto_hal_build::codegen::macros::scaffolding([#(#idents_raw,)*]).into()
        }
    }
}
