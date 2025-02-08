use std::collections::HashSet;

use darling::FromMeta;
use proc_macro2::Span;
use quote::{quote_spanned, ToTokens};
use syn::{Ident, Path};

use crate::utils::{PathArray, Spanned, SynErrorCombinator};

use super::Args;

#[derive(Debug, Clone, Default, FromMeta)]
#[darling(default)]
pub struct VariantArgs {
    #[darling(default)]
    pub bits: Option<u32>,
    pub entitlements: PathArray,

    #[darling(skip)]
    pub span: Option<Span>,
}

impl Args for VariantArgs {
    const NAME: &str = "variant";
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub args: Spanned<VariantArgs>,
    pub ident: Ident,
    pub bits: u32,
    pub entitlements: HashSet<Path>,
    pub entitlement_fields: HashSet<Ident>,
}

impl Variant {
    pub fn parse(ident: Ident, bits: u32, args: Spanned<VariantArgs>) -> syn::Result<Self> {
        let mut errors = SynErrorCombinator::new();

        let bits = args.bits.unwrap_or(bits);
        let mut entitlements = HashSet::new();
        let mut entitlement_fields = HashSet::new();

        for entitlement in args.entitlements.elems.iter().cloned() {
            entitlement_fields.insert(
                entitlement
                    .segments
                    .iter()
                    .nth_back(1)
                    .unwrap()
                    .ident
                    .clone(),
            );

            if !entitlements.insert(entitlement.clone()) {
                errors.push(syn::Error::new_spanned(
                    entitlement,
                    "entitlement already exists",
                ));
            }
        }

        errors.coalesce()?;

        Ok(Self {
            args,
            ident,
            bits,
            entitlements,
            entitlement_fields,
        })
    }
}

impl PartialEq for Variant {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
            && self.bits == other.bits
            && self.entitlements == other.entitlements
    }
}

// no validation necessary...

impl ToTokens for Variant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;

        let span = self.args.span();

        tokens.extend(quote_spanned! { span =>
            pub struct #ident {
                _sealed: (),
            }

            impl ::proto_hal::stasis::Freeze for #ident {}

            impl State for #ident {
                const RAW: ReadVariant = ReadVariant::#ident;

                unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
                }
            }
        });

        if !self.entitlements.is_empty() {
            let entitlement_paths = self.entitlements.iter();

            tokens.extend(quote_spanned! { span =>
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<super::#entitlement_paths> for #ident {}
                )*
            });
        }
    }
}
