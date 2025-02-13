use std::collections::HashMap;

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use super::field::Field;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub ident: String,
    pub offset: u32,

    pub fields: HashMap<String, Field>,
}

impl Register {
    pub fn empty(ident: String, offset: u32) -> Self {
        Self {
            ident,
            offset,
            fields: HashMap::new(),
        }
    }
}

impl PartialOrd for Register {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Register {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}

impl ToTokens for Register {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);

        let field_idents = self
            .fields
            .values()
            .map(|field| format_ident!("{}", field.ident));

        let field_bodies = self.fields.values().map(|field| field.to_token_stream());

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #field_bodies
                )*

                pub struct Reset {
                    #(
                        #field_idents: #field_idents::Reset,
                    )*
                }
            }
        });
    }
}
