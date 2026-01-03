use derive_more::{AsRef, Deref};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::{
    Node,
    diagnostic::{Context, Diagnostic, Diagnostics},
    field::{Field, FieldIndex},
    model::View,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deref)]
pub struct VariantIndex(pub(super) usize);

#[derive(Debug, Clone, Deref, AsRef)]
pub struct VariantNode {
    pub(super) parent: FieldIndex,
    #[deref]
    #[as_ref]
    pub(super) variant: Variant,
}

impl Node for VariantNode {
    type Index = VariantIndex;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub ident: Ident,
    pub bits: u32,
    pub inert: bool,
    pub docs: Vec<String>,
}

impl Variant {
    pub fn new(ident: impl AsRef<str>, bits: u32) -> Self {
        Self {
            ident: Ident::new(ident.as_ref(), Span::call_site()),
            bits,
            inert: false,
            docs: Vec::new(),
        }
    }

    pub fn inert(self) -> Self {
        Self {
            inert: true,
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

    pub fn validate(&self, context: &Context) -> Diagnostics {
        let mut diagnostics = Diagnostics::new();
        let new_context = context.clone().and(self.module_name().clone().to_string());

        // TODO: these are old...
        let reserved = ["variant", "generic", "preserve", "dynamic"]; // note: waiting for const type inference

        if reserved.contains(&self.module_name().to_string().as_str()) {
            diagnostics.insert(Diagnostic::reserved(
                &self.type_name(),
                reserved.iter(),
                new_context.clone(),
            ));
        }

        diagnostics
    }
}

// codegen
impl<'cx> View<'cx, VariantNode> {
    pub fn generate_state(&self) -> TokenStream {
        let ident = self.type_name();
        let docs = &self.docs;

        quote! {
            #(
                #[doc = #docs]
            )*
            pub struct #ident {
                _sealed: (),
            }
        }
    }

    // pub fn generate_entitlement_impls(
    //     &self,
    //     field: &Field,
    //     statewise_entitlements: Option<&EntitlementSpace>,
    // ) -> TokenStream {
    //     let ident = self.type_name();
    //     let field_ty = field.type_name();

    //     let Some(entitlements) = statewise_entitlements else {
    //         // any T satisfies this state's entitlement requirements

    //         return quote! {
    //             unsafe impl<T> ::proto_hal::stasis::Entitled<::proto_hal::stasis::entitlement_axes::Statewise, T> for #field_ty<#ident> {}
    //         };
    //     };

    //     // exactly this finite set of states satisfy this state's entitlement requirements

    //     let entitlement_paths = entitlements.iter().map(|entitlement| {
    //         let field = entitlement.field(self.model);
    //         let field_ty = field.type_name();
    //         let prefix = entitlement.render_up_to_field(self.model);
    //         let state = entitlement.render_entirely(self.model);
    //         quote! { crate::#prefix::#field_ty<crate::#state> }
    //     });

    //     quote! {
    //         #(
    //             unsafe impl ::proto_hal::stasis::Entitled<::proto_hal::stasis::entitlement_axes::Statewise, #entitlement_paths> for #field_ty<#ident> {}
    //         )*
    //     }
    // }

    pub fn generate(&self, parent: &Field) -> TokenStream {
        let mut body = quote! {};

        let statewise_entitlements = self.statewise_entitlements();

        body.extend(self.generate_state());
        // body.extend(
        //     self.generate_entitlement_impls(parent, statewise_entitlements.as_deref().copied()),
        // );

        body
    }
}
