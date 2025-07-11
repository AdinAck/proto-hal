use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::Ident;

use crate::{
    structures::entitlement::Entitlements,
    utils::diagnostic::{Context, Diagnostic, Diagnostics},
};

use super::entitlement::Entitlement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub entitlements: Entitlements,
    pub docs: Vec<String>,
}

impl Variant {
    pub fn new(ident: impl AsRef<str>, bits: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            bits,
            entitlements: Entitlements::new(),
            docs: Vec::new(),
        }
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

        let reserved = ["variant", "generic", "preserve", "dynamic"]; // note: waiting for const type inference

        if reserved.contains(&self.module_name().to_string().as_str()) {
            diagnostics.insert(
                Diagnostic::error(format!("\"{}\" is a reserved keyword", self.module_name()))
                    .notes([format!("reserved variant keywords are: {reserved:?}")])
                    .with_context(new_context.clone()),
            );
        }

        diagnostics
    }
}

// codegen
impl Variant {
    pub fn generate_state<'a>(
        ident: &Ident,
        docs: impl Iterator<Item = &'a String>,
    ) -> TokenStream {
        quote! {
            #(
                #[doc = #docs]
            )*
            pub struct #ident {
                _sealed: (),
            }

            impl #ident {
                pub fn into_dynamic(self) -> Dynamic {
                    unsafe { <Dynamic as ::proto_hal::stasis::Conjure>::conjure() }
                }
            }
        }
    }

    pub fn generate_entitlement_impls(ident: &Ident, entitlements: &Entitlements) -> TokenStream {
        if entitlements.is_empty() {
            // any T satisfies this state's entitlement requirements

            quote! {
                unsafe impl<T> ::proto_hal::stasis::Entitled<T> for #ident {}
            }
        } else {
            // exactly this finite set of states satisfy this state's entitlement requirements

            let entitlement_paths = entitlements.iter().map(|entitlement| entitlement.render());

            quote! {
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for #ident {}
                )*
            }
        }
    }

    pub fn generate_freeze_impl(ident: &Ident) -> TokenStream {
        quote! {
            impl ::proto_hal::stasis::Freeze for #ident {}
        }
    }
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = Ident::new(
            &inflector::cases::pascalcase::to_pascal_case(self.ident.to_string().as_str()),
            Span::call_site(),
        );

        tokens.extend(Self::generate_state(&ident, self.docs.iter()));
        tokens.extend(Self::generate_entitlement_impls(&ident, &self.entitlements));
        tokens.extend(Self::generate_freeze_impl(&ident));
    }
}
