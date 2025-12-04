pub mod access;
pub mod numericity;

use std::{collections::HashSet, ops::Range};

use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Ident, Index};

use crate::{
    Node,
    diagnostic::{Context, Diagnostic, Diagnostics},
    entitlement::Entitlements,
    field::{access::Access, numericity::Numericity},
    model::View,
    register::RegisterIndex,
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
    pub access: Access,
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
                let Some(hardware_write_entitlements) = self.hardware_write_entitlements() else {
                    // no entitlements means all states are entitled to,
                    // so they are exhaustive
                    None?
                };

                let mut entitlement_fields = IndexMap::new();

                for entitlement in *hardware_write_entitlements {
                    let field = entitlement.field(self.model);
                    entitlement_fields
                        .entry(field.index)
                        .or_insert_with(|| (field, HashSet::new()))
                        .1
                        .insert(entitlement.0);
                }

                // if the hardware write access entitlements are non-exhaustive
                // (meaning some states exist in which hardware does *not*
                // have write access) then the field is resolvable

                for (field, entitlements) in entitlement_fields.values() {
                    // avoid infinite recursion...
                    let enumerated = if field.index() == self.index() {
                        let Numericity::Enumerated(enumerated) = &volatile_store.numericity else {
                            unreachable!(
                                "this field must be enumerated if it produced an entitlement"
                            )
                        };

                        enumerated
                    } else {
                        let Numericity::Enumerated(enumerated) = field.resolvable().unwrap() else {
                            unreachable!("entitled field must be enumerated")
                        };

                        enumerated
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

    pub(crate) fn get_reset(&self, register_reset: u32) -> u32 {
        let mask = u32::MAX >> (32 - self.width);
        (register_reset >> self.offset) & mask
    }

    pub(crate) fn reset_ty(
        &self,
        path: &TokenStream,
        register_reset: Option<u32>,
    ) -> Option<TokenStream> {
        let Some(read) = self.access.get_read() else {
            return Some(quote! { ::proto_hal::stasis::Dynamic });
        };

        if !self.is_resolvable() {
            return Some(quote! { ::proto_hal::stasis::Dynamic });
        }

        let register_reset =
            register_reset.expect("fields which are all of: [readable, resolvable, unentitled] must have a reset value specified");

        let reset = self.get_reset(register_reset);

        match &read {
            Numericity::Numeric(numeric) => {
                let (.., ty) = numeric.ty(self.width);
                let reset = Index::from(reset as usize);

                Some(quote! { ::proto_hal::stasis::#ty<#reset> })
            }
            Numericity::Enumerated(enumerated) => {
                let ty = enumerated
                    .variants(self.model)
                    .find(|variant| variant.bits == reset)?
                    .type_name();

                Some(quote! { #path::#ty })
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

                    let variant_limit = (1u64 << self.width) - 1;

                    for variant in &sorted_variants {
                        if variant.bits as u64 > variant_limit {
                            diagnostics.insert(Diagnostic::exceeds_domain(
                                &variant.type_name(),
                                &variant.bits,
                                &format!("...0x{:x}", variant_limit),
                                new_context.clone(),
                            ));
                        }
                    }

                    // validate variant adjacency
                    for window in sorted_variants.windows(2) {
                        let lhs = &window[0];
                        let rhs = &window[1];

                        if lhs.bits == rhs.bits {
                            diagnostics.insert(Diagnostic::overlap(
                                &lhs.type_name(),
                                &rhs.type_name(),
                                &lhs.bits,
                                new_context.clone(),
                            ));
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
            diagnostics.insert(Diagnostic::read_cannot_be_inert(new_context.clone()));
        }

        // TODO: these are old...
        let reserved = ["reset", "_new_state", "_old_state"];

        if reserved.contains(&self.module_name().to_string().as_str()) {
            diagnostics.insert(Diagnostic::reserved(
                &self.module_name(),
                reserved.iter(),
                new_context.clone(),
            ));
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
    #[inline]
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
            variants.for_each(|variant| out.extend(variant.generate(self)));
        }

        out
    }

    fn generate_marker(&self, ontological_entitlements: Option<&Entitlements>) -> TokenStream {
        let entitlement_paths = ontological_entitlements.iter().flat_map(|entitlements| {
            entitlements.iter().map(|entitlement| {
                let field_ty = entitlement.field(self.model).type_name();
                let prefix = entitlement.render_up_to_field(self.model);
                let state = entitlement.render_entirely(self.model);
                quote! { crate::#prefix::#field_ty<crate::#state> }
            })
        });

        quote! {
            pub struct Field;

            #(
                unsafe impl ::proto_hal::stasis::Entitled<#entitlement_paths> for Field {}
            )*
        }
    }

    fn generate_container(&self, write_entitlements: Option<&Entitlements>) -> TokenStream {
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

        let entitlement_paths = write_entitlements.iter().flat_map(|entitlements| {
            entitlements.iter().map(|entitlement| {
                let field_ty = entitlement.field(self.model).type_name();
                let prefix = entitlement.render_up_to_field(self.model);
                let state = entitlement.render_entirely(self.model);
                quote! { crate::#prefix::#field_ty<crate::#state> }
            })
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

    fn generate_masked(
        &self,
        ontological_entitlements: Option<&Entitlements>,
    ) -> Option<TokenStream> {
        ontological_entitlements?;

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

    pub fn generate(&self) -> TokenStream {
        let ident = &self.ident;

        let ontological_entitlements = self.ontological_entitlements();
        let write_entitlements = self.write_entitlements();

        let mut body = quote! {};

        body.extend(self.generate_states());
        body.extend(self.generate_marker(ontological_entitlements.as_deref().copied()));
        body.extend(self.generate_container(write_entitlements.as_deref().copied()));
        body.extend(self.generate_repr());
        body.extend(self.generate_masked(ontological_entitlements.as_deref().copied()));
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
