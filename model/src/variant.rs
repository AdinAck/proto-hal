use derive_more::{AsRef, Deref};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    Node,
    diagnostic::{Context, Diagnostic, Diagnostics},
    entitlement::{self, generate_entitlements},
    field::{FieldIndex, FieldNode},
    model::View,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deref)]
pub struct VariantIndex(pub(super) usize);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct VariantNode {
    pub(super) parent: FieldIndex,
    #[deref]
    #[as_ref]
    pub(super) variant: Variant,
}

impl Node for VariantNode {
    type Index = VariantIndex;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub inert: bool,
    pub docs: Vec<String>,
}

impl Variant {
    pub fn new(ident: impl AsRef<str>, bits: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            bits,
            inert: false,
            docs: Vec::new(),
        }
    }

    pub fn inert(self) -> Self {
        Self {
            inert: true,
            ..self
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
        let new_context = context.clone().and(self.module_name().clone().to_string());

        // TODO: these are old...
        let reserved = ["variant", "generic", "preserve", "dynamic"]; // note: waiting for const type inference

        if reserved.contains(&self.module_name().to_string().as_str()) {
            diagnostics.insert(Diagnostic::reserved(
                &self.type_name(),
                reserved.iter(),
                new_context.clone(),
            ));
        }

        diagnostics
    }
}

// codegen
impl<'cx> View<'cx, VariantNode> {
    pub fn generate_state(&self) -> TokenStream {
        let ident = self.type_name();
        let docs = &self.docs;

        quote! {
            #(
                #[doc = #docs]
            )*
            pub struct #ident;
        }
    }

    fn generate_entitlements(
        &self,
        field: &FieldNode,
        statewise_entitlements: Option<&entitlement::Space>,
    ) -> Option<TokenStream> {
        // only proceed if *any* variant of the field has statewise entitlements
        if field
            .resolvable()
            .expect("field must be resolvable if its variants are being generated")
            .variants(self.model)
            .expect("expected field to have variants")
            .all(|variant| variant.statewise_entitlements().is_none())
        {
            None?
        }

        let ident = self.type_name();
        let field_ty = field.type_name();

        Some(match statewise_entitlements {
            Some(statewise_entitlements) if !statewise_entitlements.is_empty() => {
                let spaces = [(statewise_entitlements, entitlement::Axis::Statewise)];

                generate_entitlements(self.model, &quote! { #field_ty<#ident> }, spaces)
            }
            _ => {
                // any T satisfies this state's entitlement requirements

                quote! {
                    unsafe impl<T> ::proto_hal::stasis::Entitled<::proto_hal::stasis::patterns::Fundamental<#field_ty<#ident>, ::proto_hal::stasis::axes::Statewise>, T> for #field_ty<#ident> {}
                }
            }
        })
    }

    pub fn generate(&self, parent: &FieldNode) -> TokenStream {
        let ident = self.module_name();
        let ty = self.type_name();
        let mut body = quote! {};

        let statewise_entitlements = self.statewise_entitlements();

        body.extend(self.generate_state());
        body.extend(self.generate_entitlements(parent, statewise_entitlements.as_deref().copied()));

        quote! {
            pub mod #ident {
                #[allow(unused)]
                use super::*;

                #body
            }

            pub use #ident::#ty;
        }
    }
}
