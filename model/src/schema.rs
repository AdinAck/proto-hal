//! Schemas: shareable variant sets with a physical manifestation.
//!
//! A schema is where variants live. Its codegen manifestation is the module
//! containing the variant state types, written once at the schema's
//! *placement* — the location in the device it was inserted at. Fields
//! [assume](crate::field::access::Source::Linked) a schema by re-exporting
//! those types.
//!
//! A schema's identity is its placement *and* its name: schemas in different
//! locations may share a name freely.
//!
//! Schema nodes are topologically ordinary — they parent variants like fields
//! do — but a bit different in elaboration: their names are not path
//! segments in the modeling language, which references variants through the
//! fields that assume them.

use derive_more::{AsRef, Deref};
use heck::ToSnakeCase as _;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::{
    Node,
    field::numericity::Numericity,
    model::{Model, View},
    variant::ParentIndex,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deref)]
pub struct SchemaIndex(pub(crate) usize);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct SchemaNode {
    /// Where the schema is placed — the module its types manifest within.
    /// [`None`] places it at the device root.
    pub(crate) parent: Option<ParentIndex>,
    #[deref]
    #[as_ref]
    pub(crate) schema: Schema,
}

impl Node for SchemaNode {
    type Index = SchemaIndex;
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub ident: Ident,
    /// The variants readable through fields of this schema: plain variants
    /// and `read` variants.
    pub reads: Numericity,
    /// The variants writable through fields of this schema: plain variants
    /// and `write` variants.
    pub writes: Numericity,
    pub docs: Vec<String>,
}

impl Schema {
    pub fn ident(&self) -> Ident {
        Ident::new(&self.ident.to_string().to_snake_case(), Span::call_site())
    }

    /// The module the schema's types manifest in — always its ident.
    pub fn module_name(&self) -> Ident {
        self.ident()
    }
}

impl<'cx> View<'cx, SchemaNode> {
    /// The full path of the schema's module.
    pub fn path(&self) -> TokenStream {
        let module = self.module_name();

        match &self.parent {
            None => quote! { #module },
            Some(parent) => {
                let parent = parent.clone().path(self.model);
                quote! { #parent::#module }
            }
        }
    }
}

// codegen
impl<'cx> View<'cx, SchemaNode> {
    /// Generate the schema's module: the variant state types, defined once,
    /// and the representation enums assuming fields re-export.
    ///
    /// [`Conjure`](TODO) implementations live here — not with the fields that
    /// assume the schema — so that multiple assumptions do not produce
    /// colliding implementations.
    pub fn generate(&self) -> TokenStream {
        let module = self.module_name();

        let mut body = quote! {};

        // every variant of the schema, in declaration order — read-sided,
        // write-sided, and unsided alike — defined exactly once
        let parent = ParentIndex::Schema(self.index);
        for variant in self.model.variants_of(&parent) {
            let ty = variant.type_name();
            let docs = &variant.docs;

            body.extend(quote! {
                #(
                    #[doc = #docs]
                )*
                pub struct #ty;

                impl ::proto_hal::stasis::Conjure for #ty {
                    unsafe fn conjure() -> Self {
                        #ty
                    }
                }
            });
        }

        body.extend(self.generate_repr());

        let docs = &self.docs;

        quote! {
            #(
                #[doc = #docs]
            )*
            pub mod #module {
                #body
            }
        }
    }

    /// The representation enums, derived from the schema's content —
    /// mirroring the surface an inherent field's module exposes so assuming
    /// fields can re-export every name. No sided variants means one shared
    /// `Variant` enum; sided content splits into `ReadVariant`/`WriteVariant`.
    fn generate_repr(&self) -> TokenStream {
        match (&self.reads, &self.writes) {
            (Numericity::Enumerated(reads), Numericity::Enumerated(writes)) if reads == writes => {
                let variant_enum = variant_enum(self.model, reads, format_ident!("Variant"));

                quote! {
                    pub use Variant as ReadVariant;
                    pub use Variant as WriteVariant;

                    #variant_enum
                }
            }
            (Numericity::Enumerated(reads), Numericity::Enumerated(writes)) => {
                let reads = variant_enum(self.model, reads, format_ident!("ReadVariant"));
                let writes = variant_enum(self.model, writes, format_ident!("WriteVariant"));

                quote! {
                    #reads
                    #writes
                }
            }
            (Numericity::Enumerated(reads), Numericity::Numeric(..)) => {
                let variant_enum = variant_enum(self.model, reads, format_ident!("ReadVariant"));

                quote! {
                    pub use ReadVariant as Variant;

                    #variant_enum
                }
            }
            (Numericity::Numeric(..), Numericity::Enumerated(writes)) => {
                let variant_enum = variant_enum(self.model, writes, format_ident!("WriteVariant"));

                quote! {
                    pub use WriteVariant as Variant;

                    #variant_enum
                }
            }
            (Numericity::Numeric(..), Numericity::Numeric(..)) => quote! {},
        }
    }
}

/// A `#[repr(u32)]` enum over the provided variants.
fn variant_enum(
    model: &Model,
    enumerated: &crate::field::numericity::Enumerated,
    ident: Ident,
) -> TokenStream {
    let variant_idents = enumerated
        .variants(model)
        .map(|variant| variant.type_name())
        .collect::<Vec<_>>();
    let variant_bits = enumerated
        .variants(model)
        .map(|variant| variant.bits)
        .collect::<Vec<_>>();

    let is_variant_idents = enumerated
        .variants(model)
        .map(|variant| format_ident!("is_{}", variant.ident()));

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
}
