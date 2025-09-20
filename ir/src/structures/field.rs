use std::{collections::HashMap, fmt::Debug};

use colored::Colorize;
use inflector::Inflector;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Generics, Ident, Index, Path, Type, parse_quote};

use crate::{
    access::{Access, AccessProperties, HardwareAccess, ReadWrite},
    structures::{
        entitlement::{Entitlement, Entitlements},
        hal::Hal,
    },
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
pub enum Dimensionality {
    Single,
    Array { idents: HashMap<Ident, u8> },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub offset: u8,
    pub width: u8,
    pub access: Access,
    pub entitlements: Entitlements,
    pub hardware_access: Option<HardwareAccess>,
    pub dimensionality: Dimensionality,
    pub docs: Vec<String>,
}

impl Field {
    pub fn new(ident: impl AsRef<str>, offset: u8, width: u8, access: Access) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            offset,
            width,
            access,
            entitlements: Entitlements::new(),
            hardware_access: None,
            dimensionality: Dimensionality::Single,
            docs: Vec::new(),
        }
    }

    pub fn entitlements(mut self, entitlements: impl IntoIterator<Item = Entitlement>) -> Self {
        self.entitlements.extend(entitlements);
        self
    }

    pub fn hardware_access(self, access: HardwareAccess) -> Self {
        Self {
            hardware_access: Some(access),
            ..self
        }
    }

    pub fn array(self, count: u8, idents: impl Fn(u8) -> String + 'static) -> Self {
        Self {
            dimensionality: Dimensionality::Array {
                idents: (0..count)
                    .map(|i| (Ident::new((idents)(i).as_str(), Span::call_site()), i))
                    .collect(),
            },
            ..self
        }
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

    pub fn idents(&self) -> Vec<Ident> {
        match &self.dimensionality {
            Dimensionality::Single => vec![Ident::new(
                self.ident.to_string().to_lowercase().as_str(),
                Span::call_site(),
            )],
            Dimensionality::Array { idents } => idents
                .keys()
                .map(|ident| {
                    Ident::new(ident.to_string().to_lowercase().as_str(), Span::call_site())
                })
                .collect(),
        }
    }

    pub fn type_name(&self) -> Ident {
        Ident::new(
            self.ident.to_string().to_pascal_case().as_str(),
            Span::call_site(),
        )
    }

    pub fn type_names(&self) -> Vec<Ident> {
        match &self.dimensionality {
            Dimensionality::Single => vec![Ident::new(
                self.ident.to_string().to_pascal_case().as_str(),
                Span::call_site(),
            )],
            Dimensionality::Array { idents } => idents
                .keys()
                .map(|ident| {
                    Ident::new(
                        ident.to_string().to_pascal_case().as_str(),
                        Span::call_site(),
                    )
                })
                .collect(),
        }
    }

    pub fn is_resolvable(&self) -> bool {
        self.resolvable().is_some()
    }

    pub fn resolvable(&self) -> Option<&AccessProperties> {
        // TODO: external resolving effects nor external *unresolving* effects can currently be expressed
        // TODO: so both possibilities are ignored for now

        let hardware_access = self.hardware_access.unwrap_or(HardwareAccess::ReadOnly);

        match (&self.access, hardware_access) {
            (Access::ReadWrite(ReadWrite::Symmetrical(access)), HardwareAccess::ReadOnly) => {
                // when hardware is read only, symmetrical fields are intrinsically resolvable because:
                // 1. the values written are the values read (even numeric)
                // 2. read/write access is unconditional

                Some(access)
            }
            (
                Access::ReadWrite(ReadWrite::Asymmetrical { read, write }),
                HardwareAccess::ReadOnly,
            ) if read.numericity == write.numericity
                && !(matches!(read.numericity, Numericity::Numeric)
                    && read.entitlements.is_empty()
                    && write.entitlements.is_empty()) =>
            {
                // asymmetrical fields *can be* resolvable if:
                // 1. the read/write numericities are equal
                // 2. the numericity is not numeric with unconditional read/write access (would have been symmetrical)
                // 3. hardware access is read only

                Some(read)
            }
            _ => None,
        }
    }

    fn distribute<T>(&self, single: T, array: impl Fn(Ident) -> T) -> Vec<T> {
        match &self.dimensionality {
            Dimensionality::Single => vec![single],
            Dimensionality::Array { idents } => idents
                .keys()
                .map(|ident| {
                    Ident::new(
                        ident.to_string().to_pascal_case().as_str(),
                        Span::call_site(),
                    )
                })
                .map(array)
                .collect(),
        }
    }

    pub(crate) fn reset_tys(&self, register_reset: Option<u32>, path_prefix: Path) -> Vec<Type> {
        if !self.entitlements.is_empty() {
            return self.distribute(
                parse_quote! { Unavailable },
                |ident| parse_quote! { Unavailable::<#path_prefix::#ident> },
            );
        }

        let Some(read) = self.access.get_read() else {
            return self.distribute(
                parse_quote! { Dynamic },
                |ident| parse_quote! { Dynamic::<#path_prefix::#ident> },
            );
        };

        if !self.is_resolvable() {
            return self.distribute(
                parse_quote! { Dynamic },
                |ident| parse_quote! { Dynamic::<#path_prefix::#ident> },
            );
        }

        let register_reset =
            register_reset.expect("fields which are all of: [readable, resolvable, unentitled] must have a reset value specified");

        let mask = u32::MAX >> (32 - self.width);
        let reset = (register_reset >> self.offset) & mask;

        match &read.numericity {
            Numericity::Numeric => self.distribute(
                parse_quote! { Value::<#reset> },
                |ident| parse_quote! { Value::<#path_prefix::#ident, #reset> },
            ),
            Numericity::Enumerated { variants } => {
                let ty = variants
                    .values()
                    .find(|variant| variant.bits == reset)
                    .expect("exactly one variant must correspond to the reset value")
                    .type_name();

                self.distribute(
                    parse_quote! { #ty },
                    |ident| parse_quote! { #ty::<#path_prefix::#ident> },
                )
            }
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

                    // validate variant adjacency
                    for window in sorted_variants.windows(2) {
                        let lhs = window[0];
                        let rhs = window[1];

                        if lhs.bits == rhs.bits {
                            diagnostics.insert(
                                Diagnostic::error(format!(
                                    "variants [{}] and [{}] have overlapping bit values",
                                    lhs.ident.to_string().bold(),
                                    rhs.ident.to_string().bold()
                                ))
                                .with_context(new_context.clone()),
                            );
                        }
                    }

                    for variant in sorted_variants {
                        for entitlement in &variant.entitlements {
                            if new_context
                                .path()
                                .iter()
                                .zip([entitlement.peripheral(), entitlement.register()])
                                .take(2) // only check peripheral and register
                                .any(|(lhs, rhs)| lhs != &rhs.to_string())
                            {
                                diagnostics.insert(
                                    Diagnostic::error(
                                        "entangled variants must reside within the same register"
                                            .to_string(),
                                    )
                                    .notes([format!("erroneous entitlement: \"{entitlement}\"")])
                                    .with_context(
                                        new_context.clone().and(variant.ident.to_string()),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        };

        for access in [self.access.get_read(), self.access.get_write()]
            .into_iter()
            .flatten()
        {
            validate_numericity(&access.numericity, &mut diagnostics);

            if let Numericity::Enumerated { variants } = &access.numericity {
                for variant in variants.values() {
                    diagnostics.extend(variant.validate(&new_context));
                }
            }
        }

        // validate access entitlements
        if let (Some(read), Some(..)) = (self.access.get_read(), self.access.get_write())
            && !read.entitlements.is_empty()
        {
            diagnostics.insert(
                Diagnostic::error("writable fields cannot be conditionally readable")
                    .notes(["for more information, refer to the \"Access Entitlement Quandaries\" section in `notes.md`"])
                    .with_context(new_context.clone()),
            );
        }

        // inert is write only
        if let Some(read) = self.access.get_read()
            && let Numericity::Enumerated { variants } = &read.numericity
            && variants.values().any(|variant| variant.inert)
        {
            diagnostics.insert(
                Diagnostic::error("readable variants cannot be inert")
                    .notes([
                        "for more information, refer to the \"Inertness\" section in `notes.md`",
                    ])
                    .with_context(new_context.clone()),
            );
        }

        // TODO: this section can definitely be improved and likely has errors
        // conditional writability requires hardware write to be specified
        let ambiguous = self.access.get_read().is_some()
            && self
                .access
                .get_write()
                .is_some_and(|write| !write.entitlements.is_empty());

        if ambiguous && self.hardware_access.is_none() {
            diagnostics.insert(
                Diagnostic::error("field value retainment is ambiguous")
                    .notes(["specify the hardware field access with `.hardware_access(...)` to disambiguate how this field retains values"])
                    .with_context(new_context.clone()),
            );
        }

        if !ambiguous {
            let inferred_hardware_access = match (
                self.access.get_read().is_some(),
                self.access.get_write().is_some(),
            ) {
                (true, true) => HardwareAccess::ReadOnly,
                (true, false) => HardwareAccess::Write,
                (false, true) => HardwareAccess::ReadOnly,
                (false, false) => unreachable!(),
            };

            if let Some(hardware_access) = self.hardware_access
                && hardware_access == inferred_hardware_access
            {
                diagnostics.insert(
                Diagnostic::warning(format!("hardware access specified as {hardware_access:?} when it can be inferred as such"))
                    .with_context(new_context.clone()),
            );
            }
        }

        let reserved = ["Reset", "_NewState", "_OldState"];

        if reserved.contains(&self.type_name().to_string().as_str()) {
            diagnostics.insert(
                Diagnostic::error(format!("\"{}\" is a reserved keyword", self.type_name()))
                    .notes([format!("reserved field keywords are: {reserved:?}")])
                    .with_context(new_context.clone()),
            );
        }

        diagnostics
    }
}

// codegen
impl Field {
    fn generate_states(&self, hal: &Hal) -> TokenStream {
        // NOTE: if a field is resolvable and has split schemas,
        // the schema that represents the resolvable aspect of the
        // field must be from read access, as the value the field
        // holds must represent the state to be resolved
        //
        // NOTE: states can only be generated for the resolvable component(s)
        // of a field (since the definition of resolvability is that the state
        // it holds is statically known)

        let mut out = quote! {};

        if let Some(access) = self.resolvable()
            && let Numericity::Enumerated { variants } = &access.numericity
        {
            let variants = variants.values();
            variants.for_each(|variant| out.extend(variant.generate(self, hal)));
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
        dimensionality: &Dimensionality,
    ) -> TokenStream {
        let (generics, indicies): (Generics, _) = match dimensionality {
            Dimensionality::Single => (parse_quote! { <> }, quote! { () }),
            Dimensionality::Array { .. } => (
                parse_quote! { <Index: ::proto_hal::stasis::Index<Field>> },
                quote! { Index },
            ),
        };
        let (impl_generics, ty_generics, ..) = generics.split_for_impl();

        quote! {
            pub struct Dynamic #impl_generics {
                #(
                    #[expect(unused)] #entitlement_idents: ::proto_hal::stasis::Entitlement<#entitlement_paths>,
                )*

                _indicies: ::core::marker::PhantomData<#indicies>,
            }

            impl #impl_generics ::proto_hal::stasis::Conjure for Dynamic #ty_generics {
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #entitlement_idents: unsafe { <::proto_hal::stasis::Entitlement<#entitlement_paths> as ::proto_hal::stasis::Conjure>::conjure() },
                        )*
                        _indicies: ::core::marker::PhantomData,
                    }
                }
            }

            impl #impl_generics ::proto_hal::stasis::Position<Field> for Dynamic #ty_generics {}
            impl #impl_generics ::proto_hal::stasis::Outgoing<Field> for Dynamic #ty_generics {}
            impl #impl_generics ::proto_hal::stasis::Position<Field> for &mut Dynamic #ty_generics {}
        }
    }

    fn generate_value(&self) -> Option<TokenStream> {
        if let Some(access) = self.resolvable() {
            let Numericity::Numeric = &access.numericity else {
                None?
            };

            let ident = self.module_name();
            let (generics, indicies): (Generics, _) = match self.dimensionality {
                Dimensionality::Single => (parse_quote! { <const V: u32> }, quote! { () }),
                Dimensionality::Array { .. } => (
                    parse_quote! { <Index: ::proto_hal::stasis::Index<Field>, const V: u32> },
                    quote! { Index },
                ),
            };
            let (impl_generics, ty_generics, ..) = generics.split_for_impl();

            Some(quote! {
                pub struct Value #impl_generics {
                    _indicies: ::core::marker::PhantomData<#indicies>,
                }

                impl #impl_generics Value # ty_generics {
                    pub fn into_dynamic(self) -> Dynamic {
                        unsafe { <Dynamic as ::proto_hal::stasis::Conjure>::conjure() }
                    }

                    pub fn value(&self) -> u32 {
                        V
                    }
                }

                impl #impl_generics ::proto_hal::stasis::Conjure for Value #ty_generics {
                    unsafe fn conjure() -> Self {
                        Self {
                            _indicies: ::core::marker::PhantomData,
                        }
                    }
                }

                impl #impl_generics ::proto_hal::stasis::Emplace<super::UnsafeWriter> for Value #ty_generics {
                    fn set(&self, w: &mut super::UnsafeWriter) {
                        w.#ident(V);
                    }
                }

                impl #impl_generics ::proto_hal::stasis::Corporeal for Value #ty_generics {}
                impl #impl_generics ::proto_hal::stasis::Position<Field> for Value #ty_generics {}
                impl #impl_generics ::proto_hal::stasis::Outgoing<Field> for Value #ty_generics {}
                impl #impl_generics ::proto_hal::stasis::Incoming<Field> for Value #ty_generics {
                    type Raw = u32;
                    const RAW: Self::Raw = V;
                }
            })
        } else {
            None
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
                ReadWrite::Asymmetrical { read, write } if read.numericity == write.numericity => {
                    if let Numericity::Enumerated { variants } = &read.numericity {
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
            && let Numericity::Numeric = &write.numericity
        {
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

                impl ::proto_hal::stasis::PartialConjure for Numeric {
                    type Target = ::proto_hal::stasis::Unresolved;

                    unsafe fn partial_conjure() -> Self::Target {
                        ::proto_hal::stasis::Unresolved
                    }
                }
            });
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

    fn generate_marker_ty(entitlements: &Entitlements, hal: &Hal) -> TokenStream {
        let mut out = quote! {
            pub struct Field;
        };

        if !entitlements.is_empty() {
            let entitlement_paths = entitlements
                .iter()
                .map(|entitlement| entitlement.render(hal));

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
        dimensionality: &Dimensionality,
    ) -> TokenStream {
        let (generics, indicies): (Generics, _) = match dimensionality {
            Dimensionality::Single => (parse_quote! { <> }, quote! { () }),
            Dimensionality::Array { .. } => (
                parse_quote! { <Index: ::proto_hal::stasis::Index<Field>> },
                quote! { Index },
            ),
        };
        let (impl_generics, ty_generics, ..) = generics.split_for_impl();

        quote! {
            pub struct Unavailable #impl_generics {
                _indicies: ::core::marker::PhantomData<#indicies>,
            }

            impl #impl_generics ::proto_hal::stasis::Conjure for Unavailable #ty_generics {
                unsafe fn conjure() -> Self {
                    Self {
                        _indicies: ::core::marker::PhantomData,
                    }
                }
            }

            impl #impl_generics Unavailable #ty_generics {
                pub fn unmask(self, #(#entitlement_idents: impl Into<::proto_hal::stasis::Entitlement<#entitlement_paths>>),*) -> Dynamic #ty_generics {
                    Dynamic {
                        #(#entitlement_idents: #entitlement_idents.into(),)*
                        _indicies: ::core::marker::PhantomData,
                    }
                }
            }
        }
    }

    fn generate_indicies(&self) -> Option<TokenStream> {
        match &self.dimensionality {
            Dimensionality::Single => None,
            Dimensionality::Array { idents } => {
                let (idents, indicies) = idents
                    .iter()
                    .map(|(ident, &i)| {
                        (
                            Ident::new(
                                ident.to_string().to_pascal_case().as_str(),
                                Span::call_site(),
                            ),
                            Index::from(i as usize),
                        )
                    })
                    .collect::<(Vec<_>, Vec<_>)>();

                Some(quote! {
                    #(
                        pub struct #idents;
                        impl ::proto_hal::stasis::Index<Field> for #idents {
                            const INDEX: u32 = #indicies;
                        }
                    )*
                })
            }
        }
    }
}

// output
impl Field {
    pub fn generate(&self, hal: &Hal) -> TokenStream {
        let ident = &self.ident;

        let mut body = quote! {};

        body.extend(self.generate_states(hal));
        body.extend(Self::generate_layout_consts(
            self.offset as u32,
            self.width as u32,
        ));
        body.extend(self.generate_value());
        body.extend(Self::generate_repr(&self.ident, &self.access));
        body.extend(Self::generate_trait_impls(self));
        body.extend(Self::generate_marker_ty(&self.entitlements, hal));
        body.extend(self.generate_indicies());

        let mut entitlements = self.entitlements.iter().collect::<Vec<_>>();
        entitlements.sort_by(|lhs, rhs| lhs.field().cmp(rhs.field()));

        let entitlement_idents = entitlements
            .iter()
            .enumerate()
            .map(|(i, ..)| format_ident!("entitlement_{i}"))
            .collect::<Vec<_>>();
        let entitlement_paths = entitlements
            .iter()
            .map(|entitlement| entitlement.render(hal))
            .collect::<Vec<_>>();

        body.extend(Self::generate_dynamic(
            &entitlement_idents,
            &entitlement_paths,
            &self.dimensionality,
        ));

        if !self.entitlements.is_empty() {
            body.extend(Self::generate_unavailable(
                &entitlement_idents,
                &entitlement_paths,
                &self.dimensionality,
            ));
        }

        let docs = &self.docs;

        parse_quote! {
            #(
                #[doc = #docs]
            )*
            pub mod #ident {
                #body
            }
        }
    }
}
