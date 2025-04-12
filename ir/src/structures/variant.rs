use std::collections::HashSet;

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{parse_quote, Ident, Path};

use super::entitlement::Entitlement;

type Entitlements = HashSet<Entitlement>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub entitlements: Entitlements,
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

// codegen
impl Variant {
    pub fn generate_state(ident: &Ident) -> TokenStream {
        quote! {
            pub struct #ident {
                _sealed: (),
            }
        }
    }

    pub fn generate_state_impl(ident: &Ident) -> TokenStream {
        quote! {
            impl State for #ident {
                const RAW: ReadVariant = ReadVariant::#ident;

                unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
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

            let entitlement_paths = entitlements.iter().map(|entitlement| {
                let path = syn::parse_str::<Path>(entitlement.path()).unwrap();
                let path: Path = parse_quote! {
                    crate::#path
                };

                path
            });

            quote! {
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for #ident {}
                )*
            }
        }
    }
}

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = Ident::new(
            &inflector::cases::pascalcase::to_pascal_case(self.ident.to_string().as_str()),
            Span::call_site(),
        );

        tokens.extend(Self::generate_state(&ident));
        tokens.extend(Self::generate_state_impl(&ident));
        tokens.extend(Self::generate_entitlement_impls(&ident, &self.entitlements));
    }
}
