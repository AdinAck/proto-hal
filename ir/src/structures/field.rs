use std::collections::HashMap;

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use super::variant::Variant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numericity {
    Numeric,
    Enumerated { variants: HashMap<String, Variant> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub ident: String,
    pub offset: u8,
    pub width: u8,
    pub numericity: Numericity,
}

impl Field {
    pub fn empty(ident: String, offset: u8, width: u8, numericity: Numericity) -> Self {
        Self {
            ident,
            offset,
            width,
            numericity,
        }
    }
}

impl PartialOrd for Field {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Field {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.offset.cmp(&other.offset)
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);
        let offset = &(self.offset as u32);
        let width = &(self.width as u32);

        tokens.extend(quote! {
            pub mod #ident {
                pub const OFFSET: u32 = #offset;
                pub const WIDTH: u32 = #width;
            }
        });
    }
}
