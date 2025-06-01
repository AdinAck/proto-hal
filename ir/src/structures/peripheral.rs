use std::collections::{HashMap, HashSet};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Ident, Path};

use crate::utils::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, register::Register};

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub ident: Ident,
    pub base_addr: u32,
    pub entitlements: HashSet<Entitlement>,
    pub registers: HashMap<Ident, Register>,
}

impl Peripheral {
    pub fn new(
        ident: impl AsRef<str>,
        base_addr: u32,
        registers: impl IntoIterator<Item = Register>,
    ) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
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

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
    }

    pub fn module_name(&self) -> Ident {
        Ident::new(
            inflector::cases::snakecase::to_snake_case(self.ident.to_string().as_str()).as_str(),
            Span::call_site(),
        )
    }

    pub fn type_name(&self) -> Ident {
        Ident::new(
            inflector::cases::pascalcase::to_pascal_case(self.ident.to_string().as_str()).as_str(),
            Span::call_site(),
        )
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone().to_string());

        if self.base_addr % 4 != 0 {
            diagnostics.insert(
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
                diagnostics.insert(
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

// codegen
impl Peripheral {
    fn generate_registers<'a>(registers: impl Iterator<Item = &'a Register>) -> TokenStream {
        quote! {
            #(
                #registers
            )*
        }
    }

    fn generate_base_addr(base_addr: u32, ident: &Ident) -> TokenStream {
        let base_addr_formatted = format!("0x{:08x}", base_addr);

        let link_symbol = format!(
            "__PROTO_HAL_ADDR_OF_{}",
            inflector::cases::screamingsnakecase::to_screaming_snake_case(
                ident.to_string().as_str()
            )
        );

        quote! {
            #[cfg(not(test))]
            #[doc = #base_addr_formatted]
            pub const fn base_addr() -> usize {
                #base_addr as _
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
    }

    fn generate_reset<'a>(
        registers: impl Iterator<Item = &'a Register>,
        entitlement_idents: &Vec<Ident>,
        entitlement_paths: &Vec<Path>,
        entitlement_generic_tys: &Vec<Ident>,
    ) -> TokenStream {
        let register_idents = registers
            .filter_map(|register| {
                if register.is_resolvable() {
                    Some(register.module_name())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        quote! {
            pub struct Reset<#(#entitlement_generic_tys,)*>
            where
                #(
                    #entitlement_generic_tys: ::proto_hal::stasis::EntitlementLock<Resource = #entitlement_paths>,
                )*
            {
                #(
                    #[expect(unused)] #entitlement_idents: #entitlement_generic_tys,
                )*

                #(
                    pub #register_idents: #register_idents::Reset,
                )*
            }

            impl<#(#entitlement_generic_tys,)*> Reset<#(#entitlement_generic_tys,)*>
            where
                #(
                    #entitlement_generic_tys: ::proto_hal::stasis::EntitlementLock<Resource = #entitlement_paths>,
                )*
            {
                pub unsafe fn conjure(#(#entitlement_idents: #entitlement_generic_tys,)*) -> Self {
                    #[allow(unsafe_op_in_unsafe_fn)]
                    Self {
                        #(
                            #entitlement_idents,
                        )*
                        #(
                            #register_idents: #register_idents::Reset::conjure(),
                        )*
                    }
                }
            }
        }
    }

    fn generate_masked(
        entitlement_idents: &Vec<Ident>,
        entitlement_paths: &Vec<Path>,
        entitlement_generic_tys: &Vec<Ident>,
    ) -> TokenStream {
        quote! {
            pub struct Masked {
                _sealed: (),
            }

            impl Masked {
                pub unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
                }

                pub fn unmask<#(#entitlement_generic_tys,)*>(self, #(#entitlement_idents: #entitlement_generic_tys),*) -> Reset<#(#entitlement_generic_tys,)*>
                where
                    #(
                        #entitlement_generic_tys: ::proto_hal::stasis::EntitlementLock<Resource = #entitlement_paths>,
                    )*
                {
                    unsafe {
                        Reset::conjure(#(#entitlement_idents),*)
                    }
                }
            }
        }
    }
}

impl ToTokens for Peripheral {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut body = quote! {};

        let ident = &self.ident;

        body.extend(Self::generate_registers(self.registers.values()));
        body.extend(Self::generate_base_addr(self.base_addr, &self.ident));

        let entitlement_idents = self
            .entitlements
            .iter()
            .enumerate()
            .map(|(i, ..)| format_ident!("entitlement_{i}"))
            .collect::<Vec<_>>();
        let entitlement_paths = self
            .entitlements
            .iter()
            .map(|entitlement| entitlement.render())
            .collect::<Vec<_>>();
        let entitlement_generic_tys = (0..self.entitlements.len())
            .map(|i| format_ident!("E{i}"))
            .collect::<Vec<_>>();

        body.extend(Self::generate_reset(
            self.registers.values(),
            &entitlement_idents,
            &entitlement_paths,
            &entitlement_generic_tys,
        ));

        if !self.entitlements.is_empty() {
            body.extend(Self::generate_masked(
                &entitlement_idents,
                &entitlement_paths,
                &entitlement_generic_tys,
            ));
        }

        tokens.extend(quote! {
            pub mod #ident {
                #body
            }
        });
    }
}
