use std::collections::{HashMap, HashSet};

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{self, Context, Diagnostics};

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

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone());

        if self.base_addr % 4 != 0 {
            diagnostics.push(
                diagnostic::Error("peripheral address must be word aligned.".to_owned())
                    .with_context(new_context.clone()),
            );
        }

        for register in self.registers.values() {
            diagnostics.extend(register.validate(&new_context));
        }

        diagnostics
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
