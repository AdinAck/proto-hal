use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    diagnostic::{Context, Diagnostic, Diagnostics},
    structures::{
        Node,
        entitlement::{EntitlementIndex, Entitlements},
        hal::View,
        register::{RegisterIndex, RegisterNode},
    },
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Deref)]
pub struct PeripheralIndex(pub(super) Ident);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct PeripheralNode {
    #[deref]
    #[as_ref]
    pub(super) peripheral: Peripheral,
    pub(super) registers: IndexMap<Ident, RegisterIndex>,
}

impl Node for PeripheralNode {
    type Index = PeripheralIndex;
}

impl PeripheralNode {
    pub(super) fn add_child_index(&mut self, index: RegisterIndex, child_ident: Ident) {
        self.registers.insert(child_ident, index);
    }
}

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub ident: Ident,
    pub base_addr: u32,
    pub docs: Vec<String>,
}

impl Peripheral {
    pub fn new(ident: impl AsRef<str>, base_addr: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            base_addr,
            docs: Vec::new(),
        }
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
}

impl<'cx> View<'cx, PeripheralNode> {
    pub fn width(&self) -> u32 {
        self.registers()
            .max_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset))
            .map(|register| register.offset + 4)
            .unwrap_or(0)
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

        let mut sorted_registers = self.registers().collect::<Vec<_>>();
        sorted_registers.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for window in sorted_registers.windows(2) {
            let lhs = &window[0];
            let rhs = &window[1];

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

        for register in &sorted_registers {
            diagnostics.extend(register.validate(&new_context));
        }

        diagnostics
    }
}

// codegen
impl<'cx> View<'cx, PeripheralNode> {
    fn generate_registers(&self, registers: &Vec<View<'cx, RegisterNode>>) -> TokenStream {
        registers.iter().fold(quote! {}, |mut acc, register| {
            acc.extend(register.generate());

            acc
        })
    }

    fn generate_masked(&self, ontological_entitlements: &Entitlements) -> Option<TokenStream> {
        if ontological_entitlements.is_empty() {
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

    fn generate_reset(&self, registers: &Vec<View<'cx, RegisterNode>>) -> TokenStream {
        let register_idents = registers
            .iter()
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

impl<'cx> View<'cx, PeripheralNode> {
    pub fn generate(&self) -> TokenStream {
        let mut body = quote! {};

        let module_name = self.module_name();
        let registers = self.registers().collect();

        let ontological_entitlements = self
            .model
            .get_entitlements(EntitlementIndex::Peripheral(self.index.clone()));

        body.extend(self.generate_registers(&registers));
        body.extend(self.generate_masked(&ontological_entitlements));
        body.extend(self.generate_reset(&registers));

        let docs = &self.docs;

        quote! {
            #(#[doc = #docs])*
            #[allow(clippy::module_inception)]
            pub mod #module_name {
                #body
            }
        }
    }
}
