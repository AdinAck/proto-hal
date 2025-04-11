use std::collections::HashMap;

use colored::Colorize;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::Ident;

use crate::{
    access::Access,
    utils::diagnostic::{Context, Diagnostic, Diagnostics},
};

use super::variant::Variant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Numericity {
    Numeric,
    Enumerated { variants: HashMap<Ident, Variant> },
}

impl Numericity {
    pub fn enumerated(variants: impl IntoIterator<Item = Variant>) -> Self {
        Self::Enumerated {
            variants: HashMap::from_iter(
                variants
                    .into_iter()
                    .map(|variant| (variant.ident.clone(), variant)),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub offset: u8,
    pub width: u8,

    pub access: Access,
}

impl Field {
    pub fn new(ident: impl AsRef<str>, offset: u8, width: u8, access: Access) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            offset,
            width,
            access,
        }
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let new_context = context.clone().and(self.ident.clone().to_string());

        let validate_numericity = |numericity: &Numericity| match numericity {
            Numericity::Numeric => todo!(),
            Numericity::Enumerated { variants } => {
                let mut diagnostics = Diagnostics::new();

                if let Some(largest_variant) = variants.values().map(|variant| variant.bits).max() {
                    let variant_limit = (1 << self.width) - 1;
                    if largest_variant > variant_limit {
                        diagnostics.push(
                                Diagnostic::error(format!(
                            "field variants exceed field width. (largest variant: {}, largest possible: {})",
                            largest_variant, variant_limit,
                        ))
                                .with_context(new_context.clone()),
                            );
                    }
                }

                let mut sorted_variants = variants.values().collect::<Vec<_>>();
                sorted_variants.sort_by(|lhs, rhs| lhs.bits.cmp(&rhs.bits));

                for window in sorted_variants.windows(2) {
                    let lhs = window[0];
                    let rhs = window[1];

                    if lhs.bits == rhs.bits {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "variants [{}] and [{}] have overlapping bit values.",
                                lhs.ident.to_string().bold(),
                                rhs.ident.to_string().bold()
                            ))
                            .with_context(new_context.clone()),
                        );
                    }
                }

                diagnostics
            }
        };

        match &self.access {
            Access::Read(read) => validate_numericity(&read.numericity),
            Access::Write(write) => validate_numericity(&write.numericity),
            Access::ReadWrite { read, write } => {
                let mut diagnostics = Diagnostics::new();

                diagnostics.extend(validate_numericity(&read.numericity));
                diagnostics.extend(validate_numericity(&write.numericity));

                diagnostics
            }
        }
    }
}

// codegen
impl Field {
    pub fn generate_variant_bodies(access: &Access) -> Option<TokenStream> {
        // TODO: ???
        if let Access::Read(read) | Access::ReadWrite { read, write: _ } = access {
            if let Numericity::Enumerated { variants } = &read.numericity {
                let variants = variants.values();
                return Some(quote! { #(#variants)* });
            }
        }

        None
    }

    pub fn generate_layout_consts(offset: u32, width: u32) -> TokenStream {
        quote! {
            pub const OFFSET: u32 = #offset;
            pub const WIDTH: u32 = #width;
        }
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);

        let mut body = quote! {};

        body.extend(Self::generate_variant_bodies(&self.access));
        body.extend(Self::generate_layout_consts(
            self.offset as u32,
            self.width as u32,
        ));

        // variant enum(s)
        let variant_enum = |ident, variants: &HashMap<Ident, Variant>| {
            let variant_idents = variants
                .values()
                .map(|variant| {
                    syn::Ident::new(
                        &inflector::cases::pascalcase::to_pascal_case(
                            variant.ident.to_string().as_str(),
                        ),
                        Span::call_site(),
                    )
                })
                .collect::<Vec<_>>();
            let variant_bits = variants
                .values()
                .map(|variant| variant.bits)
                .collect::<Vec<_>>();

            let is_variant_idents = variants.values().map(|variant| {
                format_ident!(
                    "is_{}",
                    inflector::cases::snakecase::to_snake_case(variant.ident.to_string().as_str())
                )
            });

            quote! {
                #[repr(u32)]
                pub enum #ident {
                    #(
                        #variant_idents = #variant_bits,
                    )*
                }

                impl #ident {
                    pub unsafe fn from_bits(bits: u32) -> Self {
                        match bits {
                            #(
                                #variant_bits => Self::#variant_idents,
                            )*
                            _ => ::core::hint::unreachable_unchecked(),
                        }
                    }

                    #(
                        pub fn #is_variant_idents(&self) -> bool {
                            matches!(self, Self::#variant_idents)
                        }
                    )*
                }
            }
        };

        match &self.access {
            Access::Read(read) => {
                if let Numericity::Enumerated { variants } = &read.numericity {
                    let variant_enum =
                        variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                    body.extend(quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    });
                }
            }
            Access::Write(write) => {
                if let Numericity::Enumerated { variants } = &write.numericity {
                    let variant_enum =
                        variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                    body.extend(quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    });
                }
            }
            Access::ReadWrite { read, write } => {
                if read.numericity == write.numericity {
                    if let Numericity::Enumerated { variants } = &read.numericity {
                        let variant_enum =
                            variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                        body.extend(quote! {
                            pub type ReadVariant = Variant;
                            pub type WriteVariant = Variant;
                            #variant_enum
                        });
                    };
                } else {
                    if let Numericity::Enumerated { variants } = &read.numericity {
                        body.extend(variant_enum(
                            syn::Ident::new("ReadVariant", Span::call_site()),
                            variants,
                        ));
                    }

                    if let Numericity::Enumerated { variants } = &write.numericity {
                        body.extend(variant_enum(
                            syn::Ident::new("WriteVariant", Span::call_site()),
                            variants,
                        ));
                    }
                }
            }
        }

        // state trait

        if let Access::Read(read) | Access::ReadWrite { read, write: _ } = &self.access {
            if let Numericity::Enumerated { variants: _ } = &read.numericity {
                body.extend(quote! {
                    pub trait State {
                        const RAW: ReadVariant;

                        unsafe fn conjure() -> Self;
                    }
                });
            }
        }

        // final module
        tokens.extend(quote! {
            pub mod #ident {
                #body
            }
        });
    }
}
