use std::collections::HashMap;

use colored::Colorize;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Ident, Path};

use crate::{
    access::{Access, AccessProperties, ReadWrite},
    structures::entitlement::{Entitlement, Entitlements},
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
pub enum Reset {
    Variant(Ident),
    Value(u32),
}

impl From<&str> for Reset {
    fn from(ident: &str) -> Self {
        Self::Variant(Ident::new(
            inflector::cases::pascalcase::to_pascal_case(ident).as_str(),
            Span::call_site(),
        ))
    }
}

impl From<u32> for Reset {
    fn from(value: u32) -> Self {
        Self::Value(value)
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub offset: u8,
    pub width: u8,
    pub access: Access,
    pub reset: Option<Reset>,
    pub entitlements: Entitlements,
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
            entitlements: Entitlements::new(),
            docs: Vec::new(),
        }
    }

    pub fn reset(mut self, reset: impl Into<Reset>) -> Self {
        self.reset = Some(reset.into());

        self
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
            self.ident.to_string().to_lowercase().as_str(),
            Span::call_site(),
        )
    }

    pub fn type_name(&self) -> Ident {
        Ident::new(
            inflector::cases::pascalcase::to_pascal_case(self.ident.to_string().as_str()).as_str(),
            Span::call_site(),
        )
    }

    pub fn is_resolvable(&self) -> bool {
        self.resolvable().is_some()
    }

    pub fn resolvable(&self) -> Option<&AccessProperties> {
        // TODO: external resolving effects

        match &self.access {
            Access::ReadWrite(ReadWrite::Symmetrical(access)) => {
                if let Numericity::Enumerated { variants } = &access.numericity {
                    if variants.values().any(|variant| variant.inert) {
                        None
                    } else {
                        Some(access)
                    }
                } else {
                    Some(access)
                }
            }
            _ => None,
        }
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
                        let variant_limit = (1 << self.width) - 1; // note: this will break if a 32 bit enumerated field were described but that is not likely
                        if largest_variant > variant_limit {
                            diagnostics.insert(
                                Diagnostic::error(format!(
                            "field variants exceed field width. (largest variant: {largest_variant}, largest possible: {variant_limit})",
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

            if let Some(access) = self.resolvable() {
                if let Some(reset) = &self.reset {
                    // TODO: resets for resolvable fields with inequal read/write schemas
                    match reset {
                        Reset::Variant(ident) => match &access.numericity {
                            Numericity::Numeric => {
                                diagnostics.insert(
                                    Diagnostic::error(
                                        "provided reset is enumerated but field is numeric",
                                    )
                                    .with_context(new_context.clone()),
                                );
                            }
                            Numericity::Enumerated { variants } => {
                                if !variants.contains_key(ident) {
                                    diagnostics.insert(
                                        Diagnostic::error(format!(
                                            "provided reset \"{ident}\" does not exist"
                                        ))
                                        .with_context(new_context.clone()),
                                    );
                                }
                            }
                        },
                        Reset::Value(value) => match &access.numericity {
                            Numericity::Numeric => {
                                if *value > u32::MAX >> (32 - self.width) {
                                    diagnostics.insert(
                                        Diagnostic::error(format!(
                                            "provided reset value ({value}) exceeds field width"
                                        ))
                                        .with_context(new_context.clone()),
                                    );
                                }
                            }
                            Numericity::Enumerated { variants } => {
                                if !variants.values().any(|variant| variant.bits.eq(value)) {
                                    diagnostics.insert(
                                        Diagnostic::error(format!(
                                            "provided reset value ({value}) does not correspond to any variants of this field"
                                        )).with_context(new_context.clone())
                                    );
                                }
                            }
                        },
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

        let reserved = ["reset", "_new_state", "_old_state"];

        if reserved.contains(&self.module_name().to_string().as_str()) {
            diagnostics.insert(
                Diagnostic::error(format!("\"{}\" is a reserved keyword", self.module_name()))
                    .notes([format!("reserved field keywords are: {reserved:?}")])
                    .with_context(new_context.clone()),
            );
        }

        diagnostics
    }
}

// codegen
impl Field {
    fn generate_states(&self) -> TokenStream {
        // NOTE: if a field is resolvable and has split schemas,
        // the schema that represents the resolvable aspect of the
        // field must be from read access, as the value the field
        // holds must represent the state to be resolved
        //
        // NOTE: states can only be generated for the resolvable component(s)
        // of a field (since the definition of resolvability is that the state
        // it holds is statically known)

        let mut out = quote! {};

        if let Some(access) = self.resolvable() {
            if let Numericity::Enumerated { variants } = &access.numericity {
                let variants = variants.values();
                out.extend(quote! { #(#variants)* });
            }
        }

        out
    }

    fn generate_layout_consts(offset: u32, width: u32) -> TokenStream {
        quote! {
            pub const OFFSET: u32 = #offset;
            pub const WIDTH: u32 = #width;
        }
    }

    fn generate_dynamic(
        entitlement_idents: &Vec<Ident>,
        entitlement_paths: &Vec<Path>,
    ) -> TokenStream {
        quote! {
            pub struct Dynamic {
                #(
                    #[expect(unused)] #entitlement_idents: ::proto_hal::stasis::Entitlement<#entitlement_paths>,
                )*

                _sealed: (),
            }

            impl ::proto_hal::stasis::Conjure for Dynamic {
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #entitlement_idents: unsafe { <::proto_hal::stasis::Entitlement<#entitlement_paths> as ::proto_hal::stasis::Conjure>::conjure() },
                        )*
                        _sealed: (),
                    }
                }
            }

            impl ::proto_hal::stasis::Position<Field> for Dynamic {}
            impl ::proto_hal::stasis::Outgoing<Field> for Dynamic {}
            impl ::proto_hal::stasis::Position<Field> for &mut Dynamic {}
        }
    }

    fn generate_value(&self) -> Option<TokenStream> {
        if let Some(access) = self.resolvable() {
            let Numericity::Numeric = &access.numericity else {
                None?
            };

            let ident = self.module_name();

            Some(quote! {
                pub struct Value<const N: u32> {
                    _sealed: (),
                }

                impl<const N: u32> Value<N> {
                    pub fn into_dynamic(self) -> Dynamic {
                        unsafe { <Dynamic as ::proto_hal::stasis::Conjure>::conjure() }
                    }

                    pub fn value(&self) -> u32 {
                        N
                    }
                }

                impl<const N: u32> ::proto_hal::stasis::Conjure for Value<N> {
                    unsafe fn conjure() -> Self {
                        Self {
                            _sealed: (),
                        }
                    }
                }

                impl<const N: u32> ::proto_hal::stasis::Emplace<super::UnsafeWriter> for Value<N> {
                    fn set(&self, w: &mut super::UnsafeWriter) {
                        w.#ident(N);
                    }
                }

                impl<const N: u32> ::proto_hal::stasis::Corporeal for Value<N> {}
                impl<const N: u32> ::proto_hal::stasis::Position<Field> for Value<N> {}
                impl<const N: u32> ::proto_hal::stasis::Outgoing<Field> for Value<N> {}
                impl<const N: u32> ::proto_hal::stasis::Incoming<Field> for Value<N> {
                    type Raw = u32;
                    const RAW: Self::Raw = N;
                }
            })
        } else {
            None
        }
    }

    fn generate_reset(&self) -> TokenStream {
        if !self.entitlements.is_empty() {
            return quote! {
                pub type Reset = Unavailable;
            };
        }

        if let Some(reset) = &self.reset {
            let reset = match reset {
                Reset::Variant(ident) => quote! { #ident },
                Reset::Value(value) => quote! { Value<#value> },
            };

            quote! {
                pub type Reset = #reset;
            }
        } else {
            quote! {
                pub type Reset = Dynamic;
            }
        }
    }

    fn generate_repr(field_ident: &Ident, access: &Access) -> Option<TokenStream> {
        let variant_enum = |ident, variants: &HashMap<Ident, Variant>, write| {
            let variant_idents = variants
                .values()
                .map(|variant| variant.type_name())
                .collect::<Vec<_>>();
            let variant_bits = variants
                .values()
                .map(|variant| variant.bits)
                .collect::<Vec<_>>();

            let is_variant_idents = variants
                .values()
                .map(|variant| format_ident!("is_{}", variant.module_name()));

            let mut out = quote! {
                #[derive(Clone, Copy)]
                #[repr(u32)]
                pub enum #ident {
                    #(
                        #variant_idents = #variant_bits,
                    )*
                }

                impl #ident {
                    /// # Safety
                    /// If the source bits do not correspond to any variants of this field,
                    /// the behavior of any code dependent on the value of this field state
                    /// will be rendered unsound.
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
            };

            if write {
                out.extend(quote! {
                    impl ::proto_hal::stasis::Emplace<super::UnsafeWriter> for #ident {
                        fn set(&self, w: &mut super::UnsafeWriter) {
                            w.#field_ident(*self);
                        }
                    }

                    impl ::proto_hal::stasis::Position<Field> for #ident {}
                    impl ::proto_hal::stasis::Corporeal for #ident {}
                });
            }

            out
        };

        let mut out = match access {
            Access::Read(read) => {
                if let Numericity::Enumerated { variants } = &read.numericity {
                    let variant_enum = variant_enum(
                        syn::Ident::new("Variant", Span::call_site()),
                        variants,
                        false,
                    );

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
                    let variant_enum = variant_enum(
                        syn::Ident::new("Variant", Span::call_site()),
                        variants,
                        true,
                    );

                    Some(quote! {
                        pub type ReadVariant = Variant;
                        pub type WriteVariant = Variant;
                        #variant_enum
                    })
                } else {
                    None
                }
            }
            Access::ReadWrite(read_write) => match read_write {
                ReadWrite::Symmetrical(access) => {
                    if let Numericity::Enumerated { variants } = &access.numericity {
                        let variant_enum = variant_enum(
                            syn::Ident::new("Variant", Span::call_site()),
                            variants,
                            true,
                        );

                        Some(quote! {
                            pub type ReadVariant = Variant;
                            pub type WriteVariant = Variant;
                            #variant_enum
                        })
                    } else {
                        None
                    }
                }
                ReadWrite::Asymmetrical { read, write } => {
                    let read_enum = if let Numericity::Enumerated { variants } = &read.numericity {
                        Some(variant_enum(
                            syn::Ident::new("ReadVariant", Span::call_site()),
                            variants,
                            false,
                        ))
                    } else {
                        None
                    };

                    let write_enum = if let Numericity::Enumerated { variants } = &write.numericity
                    {
                        Some(variant_enum(
                            syn::Ident::new("WriteVariant", Span::call_site()),
                            variants,
                            true,
                        ))
                    } else {
                        None
                    };

                    Some(quote! {
                        #read_enum
                        #write_enum
                    })
                }
            },
        };

        if let Access::Write(write)
        | Access::ReadWrite(
            ReadWrite::Symmetrical(write) | ReadWrite::Asymmetrical { write, .. },
        ) = access
        {
            if let Numericity::Numeric = &write.numericity {
                out.get_or_insert_default().extend(quote! {
                    pub struct Numeric(u32);

                    impl ::core::ops::Deref for Numeric {
                        type Target = u32;

                        fn deref(&self) -> &Self::Target {
                            &self.0
                        }
                    }

                    impl ::core::convert::From<u32> for Numeric {
                        fn from(value: u32) -> Self {
                            Self(value)
                        }
                    }

                    impl ::proto_hal::stasis::Emplace<super::UnsafeWriter> for Numeric {
                        fn set(&self, w: &mut super::UnsafeWriter) {
                            w.#field_ident(**self);
                        }
                    }

                    impl ::proto_hal::stasis::Position<Field> for Numeric {}
                    impl ::proto_hal::stasis::Corporeal for Numeric {}
                });
            }
        }

        out
    }

    fn generate_trait_impls(&self) -> Option<TokenStream> {
        if let Some(access) = self.resolvable() {
            if let Numericity::Enumerated { variants } = &access.numericity {
                let ident = &self.ident;
                let variants = variants.values().map(|variant| variant.type_name());
                Some(quote! {
                    #(
                        impl ::proto_hal::stasis::Conjure for #variants {
                            unsafe fn conjure() -> Self {
                                Self {
                                    _sealed: (),
                                }
                            }
                        }

                        impl ::proto_hal::stasis::Emplace<super::UnsafeWriter> for #variants {
                            fn set(&self, w: &mut super::UnsafeWriter) {
                                w.#ident(<Self as ::proto_hal::stasis::Incoming<Field>>::RAW);
                            }
                        }

                        impl ::proto_hal::stasis::Corporeal for #variants {}
                        impl ::proto_hal::stasis::Position<Field> for #variants {}
                        impl ::proto_hal::stasis::Outgoing<Field> for #variants {}
                        impl ::proto_hal::stasis::Incoming<Field> for #variants {
                            type Raw = ReadVariant;
                            const RAW: Self::Raw = Self::Raw::#variants;
                        }
                    )*
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn generate_marker_ty(entitlements: &Entitlements) -> TokenStream {
        let mut out = quote! {
            pub struct Field;
        };

        if !entitlements.is_empty() {
            let entitlement_paths = entitlements.iter().map(|entitlement| entitlement.render());

            out.extend(quote! {
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for Field {}
                )*
            });
        }

        out
    }

    fn generate_unavailable(
        entitlement_idents: &Vec<Ident>,
        entitlement_paths: &Vec<Path>,
    ) -> TokenStream {
        quote! {
            pub struct Unavailable {
                _sealed: (),
            }

            impl ::proto_hal::stasis::Conjure for Unavailable {
                unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
                }
            }

            impl Unavailable {
                pub fn unmask(self, #(#entitlement_idents: impl Into<::proto_hal::stasis::Entitlement<#entitlement_paths>>),*) -> Dynamic {
                    Dynamic {
                        #(#entitlement_idents: #entitlement_idents.into(),)*
                        _sealed: (),
                    }
                }
            }
        }
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.ident;

        let mut body = quote! {};

        body.extend(self.generate_states());
        body.extend(Self::generate_layout_consts(
            self.offset as u32,
            self.width as u32,
        ));
        body.extend(self.generate_value());
        body.extend(self.generate_reset());
        body.extend(Self::generate_repr(&self.ident, &self.access));
        body.extend(Self::generate_trait_impls(self));
        body.extend(Self::generate_marker_ty(&self.entitlements));

        let mut entitlements = self.entitlements.iter().collect::<Vec<_>>();
        entitlements.sort_by(|lhs, rhs| lhs.field().cmp(rhs.field()));

        let entitlement_idents = entitlements
            .iter()
            .enumerate()
            .map(|(i, ..)| format_ident!("entitlement_{i}"))
            .collect::<Vec<_>>();
        let entitlement_paths = entitlements
            .iter()
            .map(|entitlement| entitlement.render())
            .collect::<Vec<_>>();

        body.extend(Self::generate_dynamic(
            &entitlement_idents,
            &entitlement_paths,
        ));

        if !self.entitlements.is_empty() {
            body.extend(Self::generate_unavailable(
                &entitlement_idents,
                &entitlement_paths,
            ));
        }

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
