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
                    .map(|variant| (variant.type_name(), variant)),
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
    pub docs: Vec<String>,
}

impl Field {
    pub fn new(ident: impl AsRef<str>, offset: u8, width: u8, access: Access) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            offset,
            width,
            access,
            reset: None,
            docs: Vec::new(),
        }
    }

    pub fn reset(mut self, ident: impl AsRef<str>) -> Self {
        self.reset = Some(Ident::new(
            inflector::cases::pascalcase::to_pascal_case(ident.as_ref()).as_str(),
            Span::call_site(),
        ));

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

    pub fn writer_ident(&self) -> Ident {
        format_ident!("{}Writer", self.type_name())
    }

    pub fn is_resolvable(&self) -> bool {
        // TODO: external resolving effects

        let Access::ReadWrite { read, write } = &self.access else {
            return false;
        };

        let (
            Numericity::Enumerated {
                variants: read_variants,
            },
            Numericity::Enumerated {
                variants: write_variants,
            },
        ) = (&read.numericity, &write.numericity)
        else {
            return false;
        };

        if read_variants.len() != write_variants.len() {
            return false;
        }

        for (key, variant) in read_variants.iter() {
            if write_variants
                .get(key)
                .is_none_or(|other| other.bits != variant.bits)
            {
                return false;
            }
        }

        true
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let new_context = context.clone().and(self.ident.clone().to_string());
        let mut diagnostics = Diagnostics::new();

        let validate_numericity = |numericity: &Numericity, diagnostics: &mut Diagnostics| {
            match numericity {
                Numericity::Numeric => (),
                Numericity::Enumerated { variants } => {
                    if let Some(largest_variant) =
                        variants.values().map(|variant| variant.bits).max()
                    {
                        let variant_limit = (1 << self.width) - 1;
                        if largest_variant > variant_limit {
                            diagnostics.insert(
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
                            diagnostics.insert(
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
                diagnostics.insert(
                    Diagnostic::warning("provided reset unused because the field is unresolvable")
                        .with_context(new_context.clone()),
                );
            }
        };

        for access in [self.access.get_read(), self.access.get_write()]
            .into_iter()
            .flatten()
        {
            validate_numericity(&access.numericity, &mut diagnostics);

            if self.is_resolvable() {
                if let Some(reset) = &self.reset {
                    // TODO: resets for resolvable fields with inequal read/write schemas
                    if let Numericity::Enumerated { variants } = &access.numericity {
                        if !variants.contains_key(reset) {
                            diagnostics.insert(
                                Diagnostic::error(format!(
                                    "provided reset \"{reset}\" does not exist"
                                ))
                                .with_context(new_context.clone()),
                            );
                        }
                    }
                } else {
                    diagnostics.insert(
                        Diagnostic::error(
                            "resolvable fields require a reset state to be specified",
                        )
                        .with_context(new_context.clone()),
                    );
                }
            } else {
                unused_reset(&mut diagnostics);
            }

            if let Numericity::Enumerated { variants } = &access.numericity {
                for variant in variants.values() {
                    diagnostics.extend(variant.validate(&new_context));
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

    fn generate_reset(reset: &Ident) -> TokenStream {
        quote! {
            pub type Reset = #reset;
        }
    }

    fn generate_variant_enum(access: &Access) -> Option<TokenStream> {
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

                    Some(quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    })
                } else {
                    None
                }
            }
            Access::Write(write) => {
                if let Numericity::Enumerated { variants } = &write.numericity {
                    let variant_enum =
                        variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                    Some(quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    })
                } else {
                    None
                }
            }
            Access::ReadWrite { read, write } => {
                if read.numericity == write.numericity {
                    if let Numericity::Enumerated { variants } = &read.numericity {
                        let variant_enum =
                            variant_enum(syn::Ident::new("Variant", Span::call_site()), variants);

                        Some(quote! {
                            pub type ReadVariant = Variant;
                            pub type WriteVariant = Variant;
                            #variant_enum
                        })
                    } else {
                        None
                    }
                } else {
                    let read_enum = if let Numericity::Enumerated { variants } = &read.numericity {
                        Some(variant_enum(
                            syn::Ident::new("ReadVariant", Span::call_site()),
                            variants,
                        ))
                    } else {
                        None
                    };

                    let write_enum = if let Numericity::Enumerated { variants } = &write.numericity
                    {
                        Some(variant_enum(
                            syn::Ident::new("WriteVariant", Span::call_site()),
                            variants,
                        ))
                    } else {
                        None
                    };

                    Some(quote! {
                        #read_enum
                        #write_enum
                    })
                }
            }
        }
    }

    fn generate_state_trait(access: &Access) -> Option<TokenStream> {
        if let Access::Read(read) | Access::ReadWrite { read, write: _ } = access {
            if let Numericity::Enumerated { variants: _ } = &read.numericity {
                Some(quote! {
                    pub trait State: ::proto_hal::stasis::PartialState<super::UnsafeWriter> {
                        const RAW: ReadVariant;
                    }
                })
            } else {
                todo!()
            }
        } else {
            None
        }
    }

    fn generate_state_impls(access: &Access, field_ident: &Ident) -> Option<TokenStream> {
        if let Access::Write(write) | Access::ReadWrite { read: _, write } = access {
            if let Numericity::Enumerated { variants } = &write.numericity {
                let variants = variants.values().map(|variant| variant.type_name());
                return Some(quote! {
                    #(
                        impl ::proto_hal::stasis::PartialState<super::UnsafeWriter> for #variants {
                            fn set(w: &mut super::UnsafeWriter) {
                                w.#field_ident().variant(Self::RAW);
                            }

                            unsafe fn conjure() -> Self {
                                Self {
                                    _sealed: (),
                                }
                            }
                        }

                        impl State for #variants {
                            const RAW: ReadVariant = ReadVariant::#variants;
                        }
                    )*
                });
            }
        }

        None
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;

        let mut body = quote! {};

        body.extend(Self::generate_states(&self.access));
        body.extend(Self::generate_layout_consts(
            self.offset as u32,
            self.width as u32,
        ));
        if let Some(reset) = &self.reset {
            body.extend(Self::generate_reset(&reset));
        }
        body.extend(Self::generate_variant_enum(&self.access));
        body.extend(Self::generate_state_trait(&self.access));
        body.extend(Self::generate_state_impls(&self.access, ident));

        let docs = &self.docs;

        // final module
        tokens.extend(quote! {
            #(
                #[doc = #docs]
            )*
            pub mod #ident {
                #body
            }
        });
    }
}
