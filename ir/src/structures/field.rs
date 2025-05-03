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

    pub reset: Option<Ident>,
}

impl Field {
    pub fn new(ident: impl AsRef<str>, offset: u8, width: u8, access: Access) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            offset,
            width,
            access,
            reset: None,
        }
    }

    pub fn reset(mut self, ident: impl AsRef<str>) -> Self {
        self.reset = Some(Ident::new(ident.as_ref(), Span::call_site()));

        self
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let new_context = context.clone().and(self.ident.clone().to_string());
        let mut diagnostics = Diagnostics::new();

        let validate_numericity = |numericity: &Numericity, diagnostics: &mut Diagnostics| {
            match numericity {
                Numericity::Numeric => todo!(),
                Numericity::Enumerated { variants } => {
                    if let Some(largest_variant) =
                        variants.values().map(|variant| variant.bits).max()
                    {
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
                }
            }
        };

        let unused_reset = |diagnostics: &mut Diagnostics| {
            // TODO: check for resolving effects
            if self.reset.is_some() {
                diagnostics.push(Diagnostic::warning(
                    "provided reset unused because the field is unresolvable",
                ));
            }
        };

        match &self.access {
            Access::Read(read) => {
                validate_numericity(&read.numericity, &mut diagnostics);

                unused_reset(&mut diagnostics);
            }
            Access::Write(write) => {
                validate_numericity(&write.numericity, &mut diagnostics);

                unused_reset(&mut diagnostics);
            }
            Access::ReadWrite { read, write } => {
                validate_numericity(&read.numericity, &mut diagnostics);
                validate_numericity(&write.numericity, &mut diagnostics);

                if self.reset.is_none() {
                    diagnostics.push(Diagnostic::error(
                        "resolvable fields requre a reset state to be specified",
                    ));
                }
            }
        }

        diagnostics
    }
}

// codegen
impl Field {
    fn generate_states(access: &Access) -> Option<TokenStream> {
        // NOTE: if a field is resolvable and has split schemas,
        // the schema that represents the resolvable aspect of the
        // field must be from read access, as the value the field
        // holds must represent the state to be resolved
        //
        // NOTE: states can only be generated for the resolvable component(s)
        // of a field (since the definition of resolvability is that the state
        // it holds is statically known)
        if let Access::Read(read) | Access::ReadWrite { read, write: _ } = access {
            if let Numericity::Enumerated { variants } = &read.numericity {
                let variants = variants.values();
                return Some(quote! { #(#variants)* });
            }
        }

        None
    }

    fn generate_layout_consts(offset: u32, width: u32) -> TokenStream {
        quote! {
            pub const OFFSET: u32 = #offset;
            pub const WIDTH: u32 = #width;
        }
    }

    fn generate_variant_enums(access: &Access) -> TokenStream {
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
                            _ => unsafe { ::core::hint::unreachable_unchecked() },
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

        match access {
            Access::Read(read) => {
                if let Numericity::Enumerated { variants } = &read.numericity {
                    let variant_enum =
                        variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                    quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    }
                } else {
                    todo!()
                }
            }
            Access::Write(write) => {
                if let Numericity::Enumerated { variants } = &write.numericity {
                    let variant_enum =
                        variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                    quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    }
                } else {
                    todo!()
                }
            }
            Access::ReadWrite { read, write } => {
                if read.numericity == write.numericity {
                    if let Numericity::Enumerated { variants } = &read.numericity {
                        let variant_enum =
                            variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                        quote! {
                            pub type ReadVariant = Variant;
                            pub type WriteVariant = Variant;
                            #variant_enum
                        }
                    } else {
                        todo!()
                    }
                } else {
                    let read_enum = if let Numericity::Enumerated { variants } = &read.numericity {
                        variant_enum(syn::Ident::new("ReadVariant", Span::call_site()), variants)
                    } else {
                        todo!()
                    };

                    let write_enum = if let Numericity::Enumerated { variants } = &write.numericity
                    {
                        variant_enum(syn::Ident::new("WriteVariant", Span::call_site()), variants)
                    } else {
                        todo!()
                    };

                    quote! {
                        #read_enum
                        #write_enum
                    }
                }
            }
        }
    }

    fn generate_state_trait(access: &Access) -> Option<TokenStream> {
        if let Access::Read(read) | Access::ReadWrite { read, write: _ } = access {
            if let Numericity::Enumerated { variants: _ } = &read.numericity {
                Some(quote! {
                    pub trait State {
                        const RAW: ReadVariant;

                        unsafe fn conjure() -> Self;
                    }
                })
            } else {
                todo!()
            }
        } else {
            None
        }
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = format_ident!("{}", self.ident);

        let mut body = quote! {};

        body.extend(Self::generate_states(&self.access));
        body.extend(Self::generate_layout_consts(
            self.offset as u32,
            self.width as u32,
        ));
        body.extend(Self::generate_variant_enums(&self.access));
        body.extend(Self::generate_state_trait(&self.access));

        // final module
        tokens.extend(quote! {
            pub mod #ident {
                #body
            }
        });
    }
}
