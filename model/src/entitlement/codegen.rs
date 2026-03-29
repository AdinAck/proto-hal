use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::Ident;

use crate::{
    Entitlement, Model,
    entitlement::{Axis, Space},
};

impl Entitlement {
    pub fn render_up_to_field(&self, model: &Model) -> TokenStream {
        let field = self.field(model);
        let register = model.get_register(field.parent);
        let peripheral = model.get_peripheral(register.parent.clone());

        let peripheral_ident = peripheral.module_name();
        let register_ident = register.module_name();
        let field_ident = field.module_name();

        quote! {
            #peripheral_ident::#register_ident::#field_ident
        }
    }

    pub fn render_entirely(&self, model: &Model) -> TokenStream {
        let prefix = self.render_up_to_field(model);
        let variant = self.variant(model);

        let variant_ident = variant.type_name();

        quote! { #prefix::#variant_ident }
    }

    pub fn render_in_container(&self, model: &Model) -> TokenStream {
        let path = self.render_entirely(model);
        let field = self.field(model);
        let field_ty = field.type_name();
        let (peripheral, register) = field.parents();

        let peripheral_ident = peripheral.module_name();
        let register_ident = register.module_name();
        let field_ident = field.module_name();

        quote! { crate::#peripheral_ident::#register_ident::#field_ident::#field_ty<crate::#path> }
    }
}

impl ToTokens for Axis {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Axis::Statewise => tokens.extend(quote! { Statewise }),
            Axis::Affordance => tokens.extend(quote! { Affordance }),
            Axis::Ontological => tokens.extend(quote! { Ontological }),
        }
    }
}

/// Generate the type-system representation of entitlement constraints. This involves both:
/// 1. producing patterns
/// 2. producing implementations of [`Entitled`](TODO)
///
/// The code generated takes the following form:
/// ```no_compile
/// mod _entitlements {
///     use super::*;
///
///     #( // for each pattern
///         pub struct #pattern_ty;
///         unsafe impl ::proto_hal::stasis::Pattern for #pattern_ty {
///             type Source = #source;
///             type Axis = ::proto_hal::stasis::axes::#axis;
///         }
///
///         #( // for each entitlement in the pattern
///             unsafe impl ::proto_hal::stasis::Entitled<#pattern_ty, crate::#entitlement_paths> for #source {}
///         )*
///     )*
/// }
/// ```
///
/// unless the space contains only one pattern in which the code generated will look like:
/// ```no_compile
/// mod _entitlements {
///     use super::*;
///
///
///     #( // for each entitlement in the pattern
///         unsafe impl ::proto_hal::stasis::Entitled<::proto_hal::stasis::patterns::Fundamental<#source, ::proto_hal::stasis::axes::#axis>, crate::#entitlement_paths> for #source {}
///     )*
/// }
/// ```
pub fn generate_entitlements<'a>(
    model: &Model,
    source: &TokenStream,
    spaces: impl IntoIterator<Item = (&'a Space, Axis)>,
) -> TokenStream {
    let bodies = spaces.into_iter().filter_map(|(space, axis)| {
        if space.count() > 1 {
            // generate pattern markers and entitlement impls for each pattern in the space

            Some(space.patterns().enumerate().map(|(i, pattern)| {
                let pattern_ty = pattern_ident(&axis, i);
                let entitlement_tys = pattern.entitlements().map(|e| e.render_in_container(model));

                quote! {
                    pub struct #pattern_ty;
                    unsafe impl ::proto_hal::stasis::Pattern for #pattern_ty {
                        type Source = #source;
                        type Axis = ::proto_hal::stasis::axes::#axis;
                    }

                    #(
                        unsafe impl ::proto_hal::stasis::Entitled<#pattern_ty, #entitlement_tys> for #source {}
                    )*
                }
            }).collect::<TokenStream>())
        } else if let Some(pattern) = space.patterns().next() {
            // use fundamental pattern for entitlement impls if space only contains one pattern
            // note: markers are only needed to discern between patterns of the same space, which is why that step may
            // be omitted in this case

            let entitlement_tys = pattern.entitlements().map(|e| e.render_in_container(model));

            Some(quote! {
                #(
                    unsafe impl ::proto_hal::stasis::Entitled<::proto_hal::stasis::patterns::Fundamental<#source, ::proto_hal::stasis::axes::#axis>, #entitlement_tys> for #source {}
                )*
            })
        } else {
            // nothing to do if space is empty
            None
        }
    });

    quote! {
        pub mod _entitlements {
            use super::*;

            #(#bodies)*
        }
    }
}

/// Produce a pattern identifier for the given axis and index.
///
/// For example:
/// - `StatewisePattern13`
/// - `OntologicalPattern42`
pub fn pattern_ident(axis: &Axis, index: usize) -> Ident {
    format_ident!("{}Pattern{index}", axis.to_token_stream().to_string())
}
