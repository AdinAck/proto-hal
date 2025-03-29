use std::collections::{HashMap, HashSet};

use quote::{format_ident, quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, register::Register, Ident};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peripheral {
    pub ident: String,
    pub base_addr: u32,
    pub entitlements: HashSet<Entitlement>,
    pub registers: HashMap<String, Register>,
}

impl Peripheral {
    pub fn empty(ident: impl Into<String>, base_addr: u32) -> Self {
        Self {
            ident: ident.into(),
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

    pub fn registers(mut self, registers: impl IntoIterator<Item = Register>) -> Self {
        for register in registers {
            self.registers.insert(register.ident.clone(), register);
        }

        self
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone());

        if self.base_addr % 4 != 0 {
            diagnostics.push(
                Diagnostic::error("peripheral address must be word aligned.")
                    .with_context(new_context.clone()),
            );
        }

        let mut sorted_registers = self.registers.values().collect::<Vec<_>>();

        sorted_registers.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for window in sorted_registers.windows(2) {
            let lhs = window[0];
            let rhs = window[1];

            if lhs.offset + 4 > rhs.offset {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "registers [{}] and [{}] overlap.",
                        lhs.ident, rhs.ident
                    ))
                    .with_context(new_context.clone()),
                );
            }
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

impl Ident for Peripheral {
    fn ident(&self) -> &str {
        &self.ident
    }
}

impl ToTokens for Peripheral {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);
        let base_addr = self.base_addr;
        let base_addr_formatted = format!("0x{:08x}", base_addr);

        let register_bodies = self
            .registers
            .values()
            .map(|register| register.to_token_stream());

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #register_bodies
                )*

                #[doc = #base_addr_formatted]
                pub const BASE_ADDR: u32 = #base_addr;
            }
        });
    }
}
