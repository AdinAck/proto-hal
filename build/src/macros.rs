pub mod diagnostic;
mod gates;
pub mod parsing;
mod scaffolding;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

pub use proc_macro2;
pub use syn;

pub use gates::{
    modify::{modify, modify_in_place},
    modify_untracked::modify_untracked,
    read::read,
    read_untracked::read_untracked,
    unmask::{unmask, unmask_in_place},
    write::{write, write_in_place},
    write_untracked::{write_from_reset_untracked, write_from_zero_untracked},
};
pub use scaffolding::scaffolding;

pub fn reexports(args: TokenStream) -> TokenStream {
    let idents_raw = vec![
        "modify",
        "modify_in_place",
        "modify_untracked",
        "read",
        "read_untracked",
        "unmask",
        "unmask_in_place",
        "write",
        "write_from_reset_untracked",
        "write_from_zero_untracked",
        "write_in_place",
    ];

    let idents = idents_raw
        .iter()
        .map(|name| Ident::new(name, Span::call_site()))
        .collect::<Vec<_>>();

    quote! {
        #(
            #[proc_macro]
            pub fn #idents(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
                match ::model::model(#args) {
                    Ok(model) => ::proto_hal_build::macros::#idents(&model, tokens.into()),
                    Err(e) => ::proto_hal_build::macros::syn::Error::new(
                        ::proto_hal_build::macros::proc_macro2::Span::call_site(),
                        format!("model generation failed with error: {e:?}"),
                    ).to_compile_error()
                }.into()
            }
        )*

        #[proc_macro]
        pub fn scaffolding(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
            ::proto_hal_build::macros::scaffolding([#(#idents_raw,)*]).into()
        }
    }
}
