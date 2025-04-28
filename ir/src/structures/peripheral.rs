use std::collections::{HashMap, HashSet};

use quote::{format_ident, quote, ToTokens};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, register::Register, Ident};

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub ident: String,
    pub base_addr: u32,
    pub entitlements: HashSet<Entitlement>,
    pub registers: HashMap<String, Register>,
}

impl Peripheral {
    pub fn new(
        ident: impl Into<String>,
        base_addr: u32,
        registers: impl IntoIterator<Item = Register>,
    ) -> Self {
        Self {
            ident: ident.into(),
            base_addr,
            entitlements: HashSet::new(),
            registers: HashMap::from_iter(
                registers
                    .into_iter()
                    .map(|register| (register.ident.clone(), register)),
            ),
        }
    }

    pub fn width(&self) -> u32 {
        self.registers
            .values()
            .max_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset))
            .map(|register| register.offset + 4)
            .unwrap_or(0)
    }

    #[expect(unused)]
    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        todo!()
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

        let link_symbol = format!(
            "__PROTO_HAL_ADDR_OF_{}",
            inflector::cases::screamingsnakecase::to_screaming_snake_case(
                ident.to_string().as_str()
            )
        );

        tokens.extend(quote! {
            pub mod #ident {
                #(
                    #register_bodies
                )*

                #[doc = #base_addr_formatted]
                pub const BASE_ADDR: usize = #base_addr as _;

                #[cfg(not(test))]
                pub const fn base_addr() -> usize {
                    BASE_ADDR
                }

                #[cfg(test)]
                pub fn base_addr() -> usize {
                    unsafe extern "Rust" {
                        #[link_name = #link_symbol]
                        fn addr_of() -> usize;
                    }

                    unsafe { addr_of() }
                }
            }
        });
    }
}
