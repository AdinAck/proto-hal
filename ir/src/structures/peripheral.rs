use std::collections::{HashMap, HashSet};

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use super::{entitlement::Entitlement, register::Register};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peripheral {
    pub ident: String,
    pub base_addr: u32,
    pub entitlements: HashSet<Entitlement>,
    pub registers: HashMap<String, Register>,
}

impl Peripheral {
    pub fn empty(ident: String, base_addr: u32) -> Self {
        Self {
            ident,
            base_addr,
            entitlements: HashSet::new(),
            registers: HashMap::new(),
        }
    }

    pub fn width(&self) -> u32 {
        self.registers
            .values()
            .max()
            .map(|register| register.offset + 4)
            .unwrap_or(0)
    }
}

impl PartialOrd for Peripheral {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Peripheral {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.base_addr.cmp(&other.base_addr)
    }
}

impl ToTokens for Peripheral {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);

        let register_idents = self
            .registers
            .values()
            .map(|register| format_ident!("{}", register.ident));

        let register_bodies = self
            .registers
            .values()
            .map(|register| register.to_token_stream());

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #register_bodies
                )*

                pub struct Reset {
                    #(
                        #register_idents: #register_idents::Reset,
                    )*
                }
            }
        });
    }
}
