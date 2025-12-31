use colored::Colorize;
use derive_more::{AsRef, Deref};
use indexmap::IndexMap;
use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    Node,
    diagnostic::{Context, Diagnostic, Diagnostics},
    field::{FieldIndex, FieldNode, numericity::Numericity},
    model::View,
    peripheral::PeripheralIndex,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deref)]
pub struct RegisterIndex(pub(super) usize);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct RegisterNode {
    pub(super) parent: PeripheralIndex,
    #[deref]
    #[as_ref]
    pub(super) register: Register,
    pub(super) fields: IndexMap<Ident, FieldIndex>,
}

impl Node for RegisterNode {
    type Index = RegisterIndex;
}

impl RegisterNode {
    pub(super) fn add_child_index(&mut self, index: FieldIndex, child_ident: Ident) {
        self.fields.insert(child_ident, index);
    }
}

#[derive(Debug, Clone)]
pub struct Register {
    pub ident: Ident,
    pub offset: u32,
    pub reset: Option<u32>,
    pub docs: Vec<String>,

    pub leaky: bool,
}

impl Register {
    pub fn new(ident: impl AsRef<str>, offset: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref().to_lowercase().as_str(), Span::call_site()),
            offset,
            reset: None,
            docs: Vec::new(),
            leaky: false,
        }
    }

    pub fn reset(mut self, reset: u32) -> Self {
        self.reset = Some(reset);

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

    /// Mark the fields in this register as *leaky*.
    ///
    /// This is useful when this model component is unsound because:
    /// 1. The HAL author knows the description is incomplete.
    /// 1. proto-hal is incapable of properly encapsulating
    ///    the invariances of the fields in this register.
    ///
    /// This will cause all interactions with the fields in this register to be `unsafe`.
    pub fn leaky(self) -> Self {
        Self {
            leaky: true,
            ..self
        }
    }

    pub fn module_name(&self) -> Ident {
        self.ident.clone()
    }

    pub fn type_name(&self) -> Ident {
        Ident::new(
            self.ident.to_string().to_pascal_case().as_str(),
            Span::call_site(),
        )
    }
}

impl<'cx> View<'cx, RegisterNode> {
    /// A register is resolvable if at least one field within it is resolvable.
    pub fn is_resolvable(&self) -> bool {
        self.fields().any(|field| field.is_resolvable())
    }

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.module_name().to_string());

        if !self.offset.is_multiple_of(4) {
            diagnostics.insert(
                Diagnostic::address_unaligned(
                    self.parent().base_addr + self.offset,
                    new_context.clone(),
                )
                .notes([format!(
                    "register offset is specified as {}",
                    format!("0x{:x}", self.offset).bold()
                )]),
            );
        }

        let mut sorted_fields = self.fields().collect::<Vec<_>>();
        sorted_fields.sort_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset));

        for (i, field) in sorted_fields.iter().enumerate() {
            let remaining = &sorted_fields[i + 1..];

            for other in remaining {
                if field.offset + field.width <= other.offset {
                    break;
                }

                let ontological_entitlements = field.ontological_entitlements();
                let other_ontological_entitlements = other.ontological_entitlements();

                // unfortunate workaround for `is_disjoint` behavior when sets are empty
                if let Some(lhs) = &ontological_entitlements
                    && let Some(rhs) = &other_ontological_entitlements
                    && lhs.is_disjoint(rhs)
                {
                    continue;
                }

                diagnostics.insert(
                    Diagnostic::overlap(
                        &field.module_name(),
                        &other.module_name(),
                        &format!(
                            "{}...{}",
                            field.domain().start.max(other.domain().start),
                            field.domain().end.min(other.domain().end - 1),
                        ),
                        new_context.clone(),
                    )
                    .notes(
                        if ontological_entitlements.is_some() || other_ontological_entitlements.is_some() {
                            vec![format!(
                                "overlapping fields have non-trivial intersecting entitlement spaces [{}] and [{}]",
                                ontological_entitlements.map(|x| x.iter().map(|e| e.to_string(self.model).bold().to_string()).collect::<Vec<_>>().join(", ")).unwrap_or("".to_string()),
                                other_ontological_entitlements.map(|x| x.iter().map(|e| e.to_string(self.model).bold().to_string()).collect::<Vec<_>>().join(", ")).unwrap_or("".to_string()),
                            )]
                        } else {
                            vec![]
                        },
                    ),
                );
            }
        }

        if let Some(field) = sorted_fields.last()
            && field.domain().end > 32
        {
            diagnostics.insert(Diagnostic::exceeds_domain(
                &field.module_name(),
                &format!("{}...{}", field.domain().start, field.domain().end - 1),
                &"0...31",
                new_context.clone(),
            ));
        }

        match self.reset {
            Some(reset) => {
                // every resolvable field should have a valid reset

                for field in self.fields() {
                    let Some(Numericity::Enumerated(enumerated)) = field.resolvable() else {
                        continue;
                    };

                    let field_reset = field.get_reset(reset);

                    if enumerated
                        .variants(self.model)
                        .any(|variant| variant.bits == field_reset)
                    {
                        continue;
                    }

                    diagnostics.insert(Diagnostic::invalid_reset(
                        &field,
                        enumerated.variants(self.model),
                        field_reset,
                        reset,
                        new_context.clone(),
                    ));
                }
            }
            None => {
                if self.is_resolvable() {
                    diagnostics.insert(Diagnostic::expected_reset(self, new_context.clone()));
                }
            }
        }

        for field in sorted_fields {
            diagnostics.extend(field.validate(&new_context));
        }

        diagnostics
    }
}

// codegen
impl<'cx> View<'cx, RegisterNode> {
    fn generate_fields(&self, fields: &Vec<View<'cx, FieldNode>>) -> TokenStream {
        fields.iter().fold(quote! {}, |mut acc, field| {
            acc.extend(field.generate());

            acc
        })
    }

    fn generate_reset(&self, fields: &Vec<View<'cx, FieldNode>>) -> TokenStream {
        let field_idents = fields
            .iter()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();

        let reset_tys = fields
            .iter()
            .map(|field| {
                let ident = field.module_name();
                let ty = field.type_name();

                let ontological_entitlements = field.ontological_entitlements();

                let reset_ty = if ontological_entitlements.is_none() {
                    let reset_ty = field.reset_ty(&quote! { #ident }, self.reset);

                    quote! { #ident::#ty<#reset_ty> }
                } else {
                    quote! { #ident::Masked }
                };

                quote! { #reset_ty }
            })
            .collect::<Vec<_>>();

        quote! {
            pub struct Reset {
                #(
                    pub #field_idents: #reset_tys,
                )*
            }

            impl ::proto_hal::stasis::Conjure for Reset {
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #field_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                        )*
                    }
                }
            }
        }
    }

    fn generate_dynamic(&self, fields: &Vec<View<'cx, FieldNode>>) -> TokenStream {
        let field_idents = fields
            .iter()
            .map(|field| field.module_name())
            .collect::<Vec<_>>();

        let reset_tys = fields
            .iter()
            .map(|field| {
                let ident = field.module_name();
                let ty = field.type_name();

                let ontological_entitlements = field.ontological_entitlements();

                let reset_ty = if ontological_entitlements.is_none() {
                    quote! { #ident::#ty<::proto_hal::stasis::Dynamic> }
                } else {
                    quote! { #ident::Masked }
                };

                quote! { #reset_ty }
            })
            .collect::<Vec<_>>();

        quote! {
            pub struct Dynamic {
                #(
                    pub #field_idents: #reset_tys,
                )*
            }

            impl ::proto_hal::stasis::Conjure for Dynamic {
                unsafe fn conjure() -> Self {
                    Self {
                        #(
                            #field_idents: unsafe { ::proto_hal::stasis::Conjure::conjure() },
                        )*
                    }
                }
            }
        }
    }

    pub fn generate(&self) -> TokenStream {
        let mut body = quote! {};

        let module_name = self.module_name();
        let fields = self.fields().collect();

        body.extend(self.generate_fields(&fields));
        body.extend(self.generate_reset(&fields));
        body.extend(self.generate_dynamic(&fields));

        let docs = &self.docs;
        quote! {
            #(#[doc = #docs])*
            pub mod #module_name {
                #body
            }
        }
    }
}
