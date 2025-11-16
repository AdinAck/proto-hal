pub mod access;
pub mod numericity;

use std::{collections::HashSet, ops::Range};

use colored::Colorize;
use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Index, Path, Type, parse_quote};

use crate::{
    diagnostic::{Context, Diagnostic, Diagnostics},
    structures::{
        Node,
        entitlement::{EntitlementIndex, Entitlements},
        field::{access::Access, numericity::Numericity},
        hal::View,
        register::RegisterIndex,
    },
};

use super::variant::Variant;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deref)]
pub struct FieldIndex(pub(super) usize);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct FieldNode {
    pub(super) parent: RegisterIndex,
    #[deref]
    #[as_ref]
    pub(super) field: Field,
    pub(super) access: Access,
}

impl Node for FieldNode {
    type Index = FieldIndex;
}

impl<'cx> View<'cx, FieldNode> {
    pub fn is_resolvable(&self) -> bool {
        self.resolvable().is_some()
    }

    pub fn resolvable(&self) -> Option<&Numericity> {
        // TODO: external resolving effects nor external *unresolving* effects can currently be expressed
        // TODO: so both possibilities are ignored for now

        match &self.access {
            Access::Read(..) | Access::Write(..) | Access::ReadWrite(..) => None,
            Access::Store(store) => Some(&store.numericity),
            Access::VolatileStore(volatile_store) => {
                let write_entitlements = self
                    .model
                    .get_entitlements(EntitlementIndex::Write(self.index));

                if write_entitlements.is_empty() {
                    // no entitlements means all states are entitled to,
                    // so they are exhaustive
                    None?
                }

                let mut entitlement_fields = IndexMap::new();

                for entitlement in *write_entitlements {
                    let field = entitlement.field(&self.model);
                    entitlement_fields
                        .entry(field.index)
                        .or_insert_with(|| (field, HashSet::new()))
                        .1
                        .insert(entitlement.0);
                }

                // if the write access entitlements are non-exhaustive
                // (meaning some states exist in which hardware does *not*
                // have write access) then the field is resolvable

                for (field, entitlements) in entitlement_fields.values() {
                    let Numericity::Enumerated(enumerated) = field.resolvable().unwrap() else {
                        unreachable!("entitled field must be enumerated")
                    };

                    let total = enumerated
                        .variants
                        .values()
                        .copied()
                        .collect::<HashSet<_>>();

                    if total.difference(entitlements).next().is_some() {
                        // entitlements are not exhaustive!
                        return Some(&volatile_store.numericity);
                    }
                }

                // entitlements are exhaustive, so the field state can never be static
                None
            }
        }
    }

    pub(crate) fn reset_ty(&self, path: Path, register_reset: Option<u32>) -> Type {
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

        match &read {
            Numericity::Numeric(numeric) => {
                let (.., ty) = numeric.ty(self.width);
                let reset = Index::from(reset as usize);

                parse_quote! { ::proto_hal::stasis::#ty<#reset> }
            }
            Numericity::Enumerated(enumerated) => {
                let ty = enumerated
                    .variants(self.model)
                    .find(|variant| variant.bits == reset)
                    .expect("exactly one variant must correspond to the reset value")
                    .type_name();

                parse_quote! { #path::#ty }
            }
        }
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let new_context = context.clone().and(self.ident.clone().to_string());
        let mut diagnostics = Diagnostics::new();

        let validate_numericity = |numericity: &Numericity, diagnostics: &mut Diagnostics| {
            match numericity {
                Numericity::Numeric(..) => {}
                Numericity::Enumerated(enumerated) => {
                    let mut sorted_variants = enumerated.variants(self.model).collect::<Vec<_>>();
                    sorted_variants.sort_by(|lhs, rhs| lhs.bits.cmp(&rhs.bits));

                    if let Some(largest_variant) =
                        sorted_variants.iter().map(|variant| variant.bits).max()
                    {
                        let variant_limit = (1u64 << self.width) - 1;
                        if largest_variant as u64 > variant_limit {
                            diagnostics.insert(
                                Diagnostic::error(format!(
                            "field variants exceed field width. (largest variant: {largest_variant}, largest possible: {variant_limit})",
                        ))
                                .with_context(new_context.clone()),
                            );
                        }
                    }

                    // validate variant adjacency
                    for window in sorted_variants.windows(2) {
                        let lhs = &window[0];
                        let rhs = &window[1];

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
                }
            }
        };

        for access in [self.access.get_read(), self.access.get_write()]
            .into_iter()
            .flatten()
        {
            validate_numericity(access, &mut diagnostics);

            if let Numericity::Enumerated(enumerated) = &access {
                for variant in enumerated.variants(self.model) {
                    diagnostics.extend(variant.validate(&new_context));
                }
            }
        }

        // inert doesn't make sense for read-only
        if let Some(read) = self.access.get_read()
            && !self.access.is_write()
            && let Numericity::Enumerated(enumerated) = read
            && enumerated.variants(self.model).any(|variant| variant.inert)
        {
            diagnostics.insert(
                Diagnostic::error("read-only variants cannot be inert")
                    .notes([
                        "for more information, refer to the \"Inertness\" section in `notes.md`",
                    ])
                    .with_context(new_context.clone()),
            );
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

#[derive(Debug, Clone)]
pub struct Field {
    pub ident: Ident,
    pub offset: u8,
    pub width: u8,
    pub docs: Vec<String>,
}

impl Field {
    pub fn new(ident: impl AsRef<str>, offset: u8, width: u8) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            offset,
            width,
            docs: Vec::new(),
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
            self.ident.to_string().to_pascal_case().as_str(),
            Span::call_site(),
        )
    }

    /// The domain of the parent register in which the field occupies.
    pub fn domain(&self) -> Range<u8> {
        self.offset..(self.offset + self.width)
    }
}

// codegen
impl<'cx> View<'cx, FieldNode> {
    fn generate_states(&self) -> TokenStream {
        let mut out = quote! {};

        if let Some(access) = self.resolvable()
            && let Numericity::Enumerated(enumerated) = &access
        {
            let variants = enumerated.variants(self.model);
            variants.for_each(|variant| out.extend(variant.generate(self.model, self)));
        }

        out
    }

    fn generate_container(&self, write_entitlements: &Entitlements) -> TokenStream {
        let ident = self.type_name();

        let into_dynamic = if self.is_resolvable() {
            Some(quote! {
                pub fn into_dynamic(self) -> #ident<::proto_hal::stasis::Dynamic> {
                    #ident {
                        _state: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                    }
                }
            })
        } else {
            None
        };

        let concrete_impl = if into_dynamic.is_some() {
            Some(quote! {
                impl<S> #ident<S> {
                    #into_dynamic
                }
            })
        } else {
            None
        };

        let entitlement_paths = write_entitlements.iter().map(|entitlement| {
            let field_ty = entitlement.field(self.model).type_name();
            let prefix = entitlement.render_up_to_field(self.model);
            let state = entitlement.render_entirely(self.model);
            quote! { #prefix::#field_ty<#state> }
        });

        quote! {
            pub struct #ident<S> {
                _state: S,
            }

            #concrete_impl

            impl<S> ::proto_hal::stasis::Conjure for #ident<S>
            where
                S: ::proto_hal::stasis::Conjure,
            {
                unsafe fn conjure() -> Self {
                    Self {
                        _state: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                    }
                }
            }

            #(
                unsafe impl<S> ::proto_hal::stasis::Entitled<#entitlement_paths> for #ident<S> {}
            )*
        }
    }

    fn generate_repr(&self) -> Option<TokenStream> {
        let variant_enum = |variants: Vec<&Variant>, ident| {
            let variant_idents = variants
                .iter()
                .map(|variant| variant.type_name())
                .collect::<Vec<_>>();
            let variant_bits = variants
                .iter()
                .map(|variant| variant.bits)
                .collect::<Vec<_>>();

            let is_variant_idents = variants
                .iter()
                .map(|variant| format_ident!("is_{}", variant.module_name()));

            quote! {
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
            }
        };

        match (self.access.get_read(), self.access.get_write()) {
            (Some(Numericity::Enumerated(read)), None) => {
                let variant_enum = variant_enum(
                    read.variants(self.model).map(|view| &***view).collect(),
                    format_ident!("ReadVariant"),
                );

                Some(quote! {
                    pub use ReadVariant as Variant;

                    #variant_enum
                })
            }
            (None, Some(Numericity::Enumerated(write))) => {
                let variant_enum = variant_enum(
                    write.variants(self.model).map(|view| &***view).collect(),
                    format_ident!("WriteVariant"),
                );

                Some(quote! {
                    pub use WriteVariant as Variant;

                    #variant_enum
                })
            }
            (Some(Numericity::Enumerated(read)), Some(Numericity::Enumerated(write)))
                if read == write =>
            {
                let variant_enum = variant_enum(
                    read.variants(self.model).map(|view| &***view).collect(),
                    format_ident!("Variant"),
                );

                Some(quote! {
                    pub use Variant as ReadVariant;
                    pub use Variant as WriteVariant;

                    #variant_enum
                })
            }
            (Some(Numericity::Enumerated(read)), Some(Numericity::Enumerated(write))) => {
                let read_variant_enum = variant_enum(
                    read.variants(self.model).map(|view| &***view).collect(),
                    format_ident!("ReadVariant"),
                );

                let write_variant_enum = variant_enum(
                    write.variants(self.model).map(|view| &***view).collect(),
                    format_ident!("WriteVariant"),
                );

                Some(quote! {
                    #read_variant_enum
                    #write_variant_enum
                })
            }
            (..) => None,
        }
    }

    fn generate_masked(&self, entitlements: &Entitlements) -> Option<TokenStream> {
        if entitlements.is_empty() {
            None?
        }

        Some(quote! {
            pub struct Masked {
                _sealed: (),
            }

            impl ::proto_hal::stasis::Conjure for Masked {
                unsafe fn conjure() -> Self {
                    Self {
                        _sealed: (),
                    }
                }
            }
        })
    }

    fn generate_state_impls(&self) -> Option<TokenStream> {
        let Some(numericity) = self.resolvable() else {
            None?
        };

        match numericity {
            Numericity::Numeric(numeric) => {
                let (raw_ty, ty) = numeric.ty(self.width);

                Some(quote! {
                    unsafe impl<const V: #raw_ty> ::proto_hal::stasis::State<Field> for ::proto_hal::stasis::#ty<V> {
                        const VALUE: u32 = V as _;
                    }
                })
            }
            Numericity::Enumerated(enumerated) => {
                let variant_values = enumerated.variants(self.model).map(|variant| variant.bits);
                let variants = enumerated
                    .variants(self.model)
                    .map(|variant| variant.type_name());
                Some(quote! {
                    #(
                        impl ::proto_hal::stasis::Conjure for #variants {
                            unsafe fn conjure() -> Self {
                                Self {
                                    _sealed: (),
                                }
                            }
                        }

                        unsafe impl ::proto_hal::stasis::State<Field> for #variants {
                            const VALUE: u32 = #variant_values;
                        }
                    )*
                })
            }
        }
    }
}

impl<'cx> View<'cx, FieldNode> {
    pub fn generate(&self) -> TokenStream {
        let ident = &self.ident;

        let ontological_entitlements = self
            .model
            .get_entitlements(EntitlementIndex::Field(self.index));
        let write_entitlements = self
            .model
            .get_entitlements(EntitlementIndex::Write(self.index));

        let mut body = quote! {};

        body.extend(self.generate_states());
        body.extend(self.generate_container(&write_entitlements));
        body.extend(self.generate_repr());
        body.extend(self.generate_masked(&ontological_entitlements));
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
