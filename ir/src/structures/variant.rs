use std::collections::HashSet;

use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{Ident, Path};

use super::entitlement::Entitlement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub entitlements: HashSet<Entitlement>,
}

impl Variant {
    pub fn new(ident: impl AsRef<str>, bits: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            bits,
            entitlements: HashSet::new(),
        }
    }

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
    }
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = syn::Ident::new(
            &inflector::cases::pascalcase::to_pascal_case(self.ident.to_string().as_str()),
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
