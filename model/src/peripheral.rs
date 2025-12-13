use std::ops::Range;

use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    Node,
    diagnostic::{Context, Diagnostic, Diagnostics},
    entitlement::Entitlements,
    model::View,
    register::{RegisterIndex, RegisterNode},
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Deref)]
pub struct PeripheralIndex(pub(super) Ident);

impl From<Ident> for PeripheralIndex {
    fn from(ident: Ident) -> Self {
        Self(ident)
    }
}

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

    pub partial: bool,
}

impl Peripheral {
    pub fn new(ident: impl AsRef<str>, base_addr: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            base_addr,
            docs: Vec::new(),
            partial: false,
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

    /// Mark the fields in this peripheral as *partially implemented*.
    ///
    /// This is useful when:
    /// 1. The HAL author knows the description is incomplete.
    /// 1. proto-hal is incapable of properly encapsulating
    ///    the invariances of the fields in this peripheral.
    ///
    /// This will cause all interactions with the fields in this peripheral to be `unsafe`.
    pub fn partial(self) -> Self {
        Self {
            partial: true,
            ..self
        }
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

    /// The domain of the device in which the peripheral occupies.
    #[inline]
    pub fn domain(&self) -> Range<u32> {
        self.base_addr..(self.base_addr + self.width())
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.ident.clone().to_string());

        if !self.base_addr.is_multiple_of(4) {
            diagnostics.insert(Diagnostic::address_unaligned(
                self.base_addr,
                new_context.clone(),
            ));
        }

        let mut sorted_registers = self.registers().collect::<Vec<_>>();
        sorted_registers.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for window in sorted_registers.windows(2) {
            let lhs = &window[0];
            let rhs = &window[1];

            if lhs.offset + 4 > rhs.offset {
                diagnostics.insert(Diagnostic::overlap(
                    &lhs.module_name(),
                    &rhs.module_name(),
                    &format!("0x{:x}...0x{:x}", rhs.offset, lhs.offset + 3),
                    new_context.clone(),
                ));
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

    fn generate_masked(
        &self,
        ontological_entitlements: Option<&Entitlements>,
    ) -> Option<TokenStream> {
        ontological_entitlements?;

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

    fn generate_entitlement_impls(
        &self,
        ontological_entitlements: Option<&Entitlements>,
    ) -> Option<TokenStream> {
        let ontological_entitlements = ontological_entitlements?;

        let entitlement_paths = ontological_entitlements.iter().map(|entitlement| {
            let field = entitlement.field(self.model);
            let field_ty = field.type_name();
            let prefix = entitlement.render_up_to_field(self.model);
            let state = entitlement.render_entirely(self.model);
            quote! { crate::#prefix::#field_ty<crate::#state> }
        });

        Some(quote! {
            #(
                unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for Reset {}
            )*
        })
    }
}

impl<'cx> View<'cx, PeripheralNode> {
    pub fn generate(&self) -> TokenStream {
        let mut body = quote! {};

        let module_name = self.module_name();
        let registers = self.registers().collect();

        let ontological_entitlements = self.ontological_entitlements();

        body.extend(self.generate_registers(&registers));
        body.extend(self.generate_masked(ontological_entitlements.as_deref().copied()));
        body.extend(self.generate_reset(&registers));
        body.extend(self.generate_entitlement_impls(ontological_entitlements.as_deref().copied()));

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
