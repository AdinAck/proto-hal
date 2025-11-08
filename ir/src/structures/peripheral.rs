use indexmap::{IndexMap, IndexSet};
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::diagnostic::{Context, Diagnostic, Diagnostics};

use super::{entitlement::Entitlement, register::Register};

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub ident: Ident,
    pub base_addr: u32,
    pub entitlements: IndexSet<Entitlement>,
    pub registers: IndexMap<Ident, Register>,
    pub docs: Vec<String>,
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
            entitlements: IndexSet::new(),
            registers: IndexMap::from_iter(
                registers
                    .into_iter()
                    .map(|register| (register.ident.clone(), register)),
            ),
            docs: Vec::new(),
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

    pub fn docs<I>(mut self, docs: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.docs
            .extend(docs.into_iter().map(|doc| doc.as_ref().to_string()));

        self
    }

    pub fn module_name(&self) -> Ident {
        self.ident.clone()
    }

    pub fn type_name(&self) -> Ident {
        Ident::new(
            self.ident.to_string().to_pascal_case().as_str(),
            Span::call_site(),
        )
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone().to_string());

        if !self.base_addr.is_multiple_of(4) {
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
    fn generate_registers(&self) -> TokenStream {
        self.registers
            .values()
            .fold(quote! {}, |mut acc, register| {
                acc.extend(register.generate(self));

                acc
            })
    }

    fn generate_base_addr(base_addr: u32) -> TokenStream {
        quote! {
            pub const BASE_ADDR: u32 = #base_addr;
        }
    }

    fn generate_masked(&self) -> Option<TokenStream> {
        if self.entitlements.is_empty() {
            None?
        }

        Some(quote! {
            pub struct Masked {
                _sealed: (),
            }

            impl ::proto_hal::stasis::Conjure for Masked {
                unsafe fn conjure() -> Self {
                    Self { _sealed: () }
                }
            }
        })
    }

    fn generate_reset<'a>(registers: impl Iterator<Item = &'a Register>) -> TokenStream {
        let register_idents = registers
            .map(|register| register.module_name())
            .collect::<Vec<_>>();

        quote! {
            pub struct Reset {
                #(
                    pub #register_idents: #register_idents::Reset,
                )*
            }

            impl ::proto_hal::stasis::Conjure for Reset {
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #register_idents: unsafe { <#register_idents::Reset as ::proto_hal::stasis::Conjure>::conjure() },
                        )*
                    }
                }
            }
        }
    }
}

impl Peripheral {
    pub fn generate(&self) -> TokenStream {
        let mut body = quote! {};

        let ident = self.module_name();

        body.extend(self.generate_registers());
        body.extend(Self::generate_base_addr(self.base_addr));
        body.extend(self.generate_masked());
        body.extend(Self::generate_reset(self.registers.values()));

        let docs = &self.docs;

        quote! {
            #(#[doc = #docs])*
            #[allow(clippy::module_inception)]
            pub mod #ident {
                #body
            }
        }
    }
}
