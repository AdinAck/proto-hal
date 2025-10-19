use colored::Colorize;
use indexmap::{IndexMap, IndexSet};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Path, Type, parse_quote};

use crate::{
    access::{Access, AccessProperties, HardwareAccess, ReadWrite},
    structures::entitlement::{Entitlement, Entitlements},
    utils::diagnostic::{Context, Diagnostic, Diagnostics},
};

use super::variant::Variant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Numericity {
    Numeric,
    Enumerated { variants: IndexMap<Ident, Variant> },
}

impl Numericity {
    pub fn enumerated(variants: impl IntoIterator<Item = Variant>) -> Self {
        Self::Enumerated {
            variants: IndexMap::from_iter(
                variants
                    .into_iter()
                    .map(|variant| (variant.type_name(), variant)),
            ),
        }
    }

    /// View an inert variant if one exists. If there is more than one, the variant returned
    /// is not guaranteed to be any particular one, nor consistent. If the numericity is
    /// [`Numeric`](Numericity::Numeric), [`None`] is returned.
    pub fn some_inert(&self) -> Option<&Variant> {
        let Numericity::Enumerated { variants } = self else {
            None?
        };
        variants.values().find(|variant| variant.inert)
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub offset: u8,
    pub width: u8,
    pub access: Access,
    pub entitlements: Entitlements,
    pub hardware_access: Option<HardwareAccess>,
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

    pub(crate) fn reset_ty(&self, path: Path, register_reset: Option<u32>) -> Type {
        if !self.entitlements.is_empty() {
            return parse_quote! { ::proto_hal::stasis::Unavailable };
        }

        let Some(read) = self.access.get_read() else {
            return parse_quote! { ::proto_hal::stasis::Dynamic };
        };

        if !self.is_resolvable() {
            return parse_quote! { ::proto_hal::stasis::Dynamic };
        }

        let register_reset =
            register_reset.expect("fields which are all of: [readable, resolvable, unentitled] must have a reset value specified");

        let mask = u32::MAX >> (32 - self.width);
        let reset = (register_reset >> self.offset) & mask;

        match &read.numericity {
            Numericity::Numeric => parse_quote! { ::proto_hal::stasis::Value<#reset> },
            Numericity::Enumerated { variants } => {
                let ty = variants
                    .values()
                    .find(|variant| variant.bits == reset)
                    .expect("exactly one variant must correspond to the reset value")
                    .type_name();

                parse_quote! { #path::#ty }
            }
        }
    }

    /// All of this field's entitlements from any of:
    /// - Field
    /// - Access
    /// - Variants
    pub fn all_entitlements(&self) -> Entitlements {
        let field_entitlements = &self.entitlements;
        // skip read; won't have entitlements (see branch "access")
        let access_entitlements = self.access.get_write().map(|write| &write.entitlements);

        let entitlements_from_numericity = |numericity: &Numericity| {
            Some(match numericity {
                Numericity::Numeric => None?,
                Numericity::Enumerated { variants } => variants
                    .values()
                    .map(|variant| &variant.entitlements)
                    .fold(IndexSet::new(), |mut acc, entitlements| {
                        acc.extend(entitlements.iter().cloned());
                        acc
                    }),
            })
        };

        let read_variant_entitlements = self
            .access
            .get_read()
            .and_then(|read| entitlements_from_numericity(&read.numericity));

        let write_variant_entitlements = self
            .access
            .get_write()
            .and_then(|write| entitlements_from_numericity(&write.numericity));

        let mut entitlements = Entitlements::new();
        entitlements.extend(field_entitlements.into_iter().cloned());
        entitlements.extend(access_entitlements.into_iter().flatten().cloned());
        entitlements.extend(read_variant_entitlements.into_iter().flatten());
        entitlements.extend(write_variant_entitlements.into_iter().flatten());

        entitlements
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
                                .zip(
                                    entitlement
                                        .render()
                                        .segments
                                        .iter()
                                        .skip(1) // skip "crate"
                                        .map(|segment| &segment.ident),
                                )
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

        if let Some(access) = self.resolvable()
            && let Numericity::Enumerated { variants } = &access.numericity
        {
            let variants = variants.values();
            variants.for_each(|variant| out.extend(variant.generate()));
        }

        out
    }

    fn generate_markers(offset: u8, width: u8) -> TokenStream {
        quote! {
            pub struct Field;
            pub const OFFSET: u8 = #offset;
            pub const WIDTH: u8 = #width;
        }
    }

    fn generate_container(&self) -> TokenStream {
        let ident = self.type_name();

        let into_dynamic = if self.is_resolvable() {
            Some(quote! {
                impl<S> #ident<S>
                where
                    S: ::proto_hal::stasis::State<Field>,
                {
                    pub fn into_dynamic(self) -> #ident<::proto_hal::stasis::Dynamic> {
                        Self {
                            _state: unsafe { <::proto_hal::stasis::Dynamic as ::proto_hal::stasis::Conjure>::conjure() },
                        }
                    }
                }
            })
        } else {
            None
        };

        quote! {
            pub struct #ident<S>
            where
                S: ::proto_hal::stasis::State<Field>,
            {
                _state: S,
            }

            #into_dynamic

            impl<S> ::proto_hal::stasis::Conjure for #ident<S>
            where
                S: ::proto_hal::stasis::State<Field>,
            {
                unsafe fn conjure() -> Self {
                    Self {
                        _state: unsafe { <S as ::proto_hal::stasis::Conjure>::conjure() },
                    }
                }
            }
        }
    }

    fn generate_repr(access: &Access) -> Option<TokenStream> {
        let variant_enum = |variants: &IndexMap<Ident, Variant>| {
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

            quote! {
                #[derive(Clone, Copy)]
                #[repr(u32)]
                pub enum Variant {
                    #(
                        #variant_idents = #variant_bits,
                    )*
                }

                impl Variant {
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
            }
        };

        match access {
            Access::Read(read) => {
                if let Numericity::Enumerated { variants } = &read.numericity {
                    let variant_enum = variant_enum(variants);

                    Some(quote! {
                        pub mod read {
                            #variant_enum
                        }
                    })
                } else {
                    None
                }
            }
            Access::Write(write) => {
                if let Numericity::Enumerated { variants } = &write.numericity {
                    let variant_enum = variant_enum(variants);

                    Some(quote! {
                        pub mod write {
                            #variant_enum
                        }
                    })
                } else {
                    None
                }
            }
            Access::ReadWrite(read_write) => match read_write {
                ReadWrite::Symmetrical(access) => {
                    if let Numericity::Enumerated { variants } = &access.numericity {
                        let variant_enum = variant_enum(variants);

                        Some(quote! {
                            #variant_enum

                            pub mod read {
                                pub use super::Variant;
                            }

                            pub mod write {
                                pub use super::Variant;
                            }
                        })
                    } else {
                        None
                    }
                }
                ReadWrite::Asymmetrical { read, write } if read.numericity == write.numericity => {
                    if let Numericity::Enumerated { variants } = &read.numericity {
                        let variant_enum = variant_enum(variants);

                        Some(quote! {
                            #variant_enum

                            pub mod read {
                                pub use super::Variant;
                            }

                            pub mod write {
                                pub use super::Variant;
                            }
                        })
                    } else {
                        None
                    }
                }
                ReadWrite::Asymmetrical { read, write } => {
                    let read_enum = if let Numericity::Enumerated { variants } = &read.numericity {
                        Some(variant_enum(variants))
                    } else {
                        None
                    };

                    let write_enum = if let Numericity::Enumerated { variants } = &write.numericity
                    {
                        Some(variant_enum(variants))
                    } else {
                        None
                    };

                    Some(quote! {
                        pub mod read {
                            #read_enum
                        }

                        pub mod write {
                            #write_enum
                        }
                    })
                }
            },
        }
    }

    fn generate_state_impls(&self) -> Option<TokenStream> {
        if let Some(access) = self.resolvable() {
            if let Numericity::Enumerated { variants } = &access.numericity {
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

                        impl ::proto_hal::stasis::State<Field> for #variants {}
                    )*
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl Field {
    pub fn generate(&self) -> TokenStream {
        let ident = &self.ident;

        let mut body = quote! {};

        body.extend(self.generate_states());
        body.extend(Self::generate_markers(self.offset, self.width));
        body.extend(self.generate_container());
        body.extend(Self::generate_repr(&self.access));
        body.extend(self.generate_state_impls());

        let docs = &self.docs;

        // final module
        quote! {
            #(
                #[doc = #docs]
            )*
            pub mod #ident {
                #body
            }
        }
    }
}
