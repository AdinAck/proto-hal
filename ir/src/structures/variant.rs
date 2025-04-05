use std::collections::HashSet;

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::Path;

use super::{entitlement::Entitlement, Ident};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: String,
    pub bits: u32,
    pub entitlements: HashSet<Entitlement>,
}

impl Variant {
    pub fn new(ident: impl Into<String>, bits: u32) -> Self {
        Self {
            ident: ident.into(),
            bits,
            entitlements: HashSet::new(),
        }
    }

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
    }
}

impl Ident for Variant {
    fn ident(&self) -> &str {
        &self.ident
    }
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = syn::Ident::new(
            &inflector::cases::pascalcase::to_pascal_case(self.ident()),
            Span::call_site(),
        );

        tokens.extend(quote! {
            pub struct #ident {
                _sealed: (),
            }

            impl State for #ident {
                const RAW: ReadVariant = ReadVariant::#ident;

                unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
                }
            }
        });

        if self.entitlements.is_empty() {
            tokens.extend(quote! {
                unsafe impl<T> ::proto_hal::stasis::Entitled<T> for #ident {}
            });
        } else {
            for entitlement in &self.entitlements {
                let entitlement_path =
                    syn::parse_str::<Path>(&format!("crate::{}", entitlement.path())).unwrap();

                tokens.extend(quote! {
                    unsafe impl ::proto_hal::stasis::Entitled<#entitlement_path> for #ident {}
                });
            }
        }
    }
}
