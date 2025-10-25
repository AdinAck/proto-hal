use inflector::Inflector;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    structures::{entitlement::Entitlements, field::Field},
    utils::diagnostic::{Context, Diagnostic, Diagnostics},
};

use super::entitlement::Entitlement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub inert: bool,
    pub entitlements: Entitlements,
    pub docs: Vec<String>,
}

impl Variant {
    pub fn new(ident: impl AsRef<str>, bits: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            bits,
            inert: false,
            entitlements: Entitlements::new(),
            docs: Vec::new(),
        }
    }

    pub fn inert(self) -> Self {
        Self {
            inert: true,
            ..self
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
    pub fn generate_state(&self) -> TokenStream {
        let ident = self.type_name();
        let docs = &self.docs;

        quote! {
            #(
                #[doc = #docs]
            )*
            pub struct #ident {
                _sealed: (),
            }
        }
    }

    pub fn generate_entitlement_impls(&self, field: &Field) -> TokenStream {
        let ident = self.type_name();
        let entitlements = &self.entitlements;
        let field_ty = field.type_name();

        if entitlements.is_empty() {
            // any T satisfies this state's entitlement requirements

            quote! {
                unsafe impl<T> ::proto_hal::stasis::Entitled<T> for #field_ty<#ident> {}
            }
        } else {
            // exactly this finite set of states satisfy this state's entitlement requirements

            let entitlement_paths = entitlements.iter().map(|entitlement| {
                let field_ty = Ident::new(
                    entitlement.field().to_string().to_pascal_case().as_str(),
                    Span::call_site(),
                );
                let prefix = entitlement.render_up_to_field();
                let state = entitlement.render_entirely();
                quote! { #prefix::#field_ty<#state> }
            });

            quote! {
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for #field_ty<#ident> {}
                )*
            }
        }
    }
}

impl Variant {
    pub fn generate(&self, parent: &Field) -> TokenStream {
        let mut body = quote! {};

        body.extend(self.generate_state());
        body.extend(self.generate_entitlement_impls(parent));

        body
    }
}
