use indexmap::IndexSet;
use ir::structures::field::{Field, Numericity};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Ident, Path};

use crate::codegen::macros::{
    gates::fragments,
    parsing::{
        semantic::{
            self,
            policies::{Filter, RequireBinding},
        },
        syntax::Binding,
    },
};

pub fn constraints<'cx, PeripheralPolicy>(
    input: &semantic::Gate<'cx, PeripheralPolicy, RequireBinding<'cx>>,
    register_path: &Path,
    binding: &Binding,
    field_ident: &Ident,
    field: &Field,
    input_generic: Option<&Ident>,
    output_generic: Option<&Ident>,
    input_ty: &TokenStream,
    return_ty: Option<&TokenStream>,
) -> Option<TokenStream>
where
    PeripheralPolicy: Filter,
{
    // if the subject field's write access has entitlements, the entitlements
    // must be satisfied in the input to the gate, and the fields used to
    // satisfy the entitlements cannot be written

    let mut constraints = Vec::new();
    let span = field_ident.span();

    if let Some(generic) = input_generic {
        constraints.push(
            quote_spanned! { span => #generic: ::proto_hal::stasis::State<#register_path::#field_ident::Field> },
        );
    }

    if let Some(generic) = output_generic {
        constraints.push(
            quote_spanned! { span => #generic: ::proto_hal::stasis::State<#register_path::#field_ident::Field> },
        );
    }

    if binding.is_mutated() {
        let write_access_entitlements = field
            .access
            .get_write()
            .map(|write| {
                write
                    .entitlements
                    .iter()
                    .map(|entitlement| {
                        (
                            entitlement.peripheral(),
                            entitlement.register(),
                            entitlement.field(),
                        )
                    })
                    .collect::<IndexSet<_>>()
            })
            .into_iter()
            .flatten()
            .filter_map(
                |(
                    entitlement_peripheral_ident,
                    entitlement_register_ident,
                    entitlement_field_ident,
                )| {
                    // note: write entitlements can only be satisfied by input types
                    // and the validation step is responsible for forbidding
                    // transitioning of entitlement fields which are write access dependencies

                    let (entitlement_register_item, entitlement_field_item) = input.get_field(
                        entitlement_peripheral_ident.to_string(),
                        entitlement_register_ident.to_string(),
                        entitlement_field_ident.to_string(),
                    )?;

                    let (entitlement_input_generic, ..) =
                        fragments::generics(entitlement_register_item, entitlement_field_item);

                    let entitlement_input_ty = fragments::input_ty(
                        &entitlement_register_item.path(),
                        entitlement_field_item.ident(),
                        entitlement_field_item.field(),
                        entitlement_input_generic.as_ref(),
                    );

                    Some(quote_spanned! { span =>
                        #input_ty: ::proto_hal::stasis::Entitled<#entitlement_input_ty>
                    })
                },
            );

        constraints.extend(write_access_entitlements);
    }

    if binding.is_viewed() || binding.is_dynamic() {
        return Some(quote_spanned! { span => #(#constraints,)* });
    }

    let Some(return_ty) = return_ty else {
        return Some(quote_spanned! { span => #(#constraints,)* });
    };

    let statewise_entitlements = field
        .access
        .get_write()
        .and_then(|write| {
            Some(match &write.numericity {
                Numericity::Numeric => None?,
                Numericity::Enumerated { variants } => variants
                    .values()
                    .flat_map(|variant| {
                        variant.entitlements.iter().map(|entitlement| {
                            (
                                entitlement.peripheral(),
                                entitlement.register(),
                                entitlement.field(),
                            )
                        })
                    })
                    .collect::<IndexSet<_>>(),
            })
        })
        .into_iter()
        .flatten()
        .filter_map(
            |(
                entitlement_peripheral_ident,
                entitlement_register_ident,
                entitlement_field_ident,
            )| {
                // there are two sides, the entitlement *holder*, and the entitlement *provider*.
                // the LHS field has variants which are entitled to variants of the RHS field.
                //
                // the LHS field is one of:
                // 1. transitioned concretely (bound on return ty)
                // 2. transitioned generically (bound on output generic)
                //
                // the RHS field is one of:
                // 1. viewed (bound on input ty)
                // 2. transitioned concretely (bound on return ty)
                // 3. transitioned generically (bound on output generic)

                // query for the entitlement field
                let (entitlement_register_item, entitlement_field_item) = input.get_field(
                    entitlement_peripheral_ident.to_string(),
                    entitlement_register_ident.to_string(),
                    entitlement_field_ident.to_string(),
                )?;

                let (entitlement_input_generic, entitlement_output_generic) =
                    fragments::generics(entitlement_register_item, entitlement_field_item);

                let entitlement_return_ty = fragments::transition_return_ty(
                    &entitlement_register_item.path(),
                    entitlement_field_item.entry(),
                    entitlement_field_item.field(),
                    entitlement_field_item.ident(),
                    entitlement_output_generic.as_ref(),
                );

                let lhs = return_ty.clone();
                let rhs = entitlement_return_ty.unwrap_or(fragments::input_ty(
                    &entitlement_register_item.path(),
                    entitlement_field_item.ident(),
                    entitlement_field_item.field(),
                    entitlement_input_generic.as_ref(),
                ));

                Some(quote_spanned! { span =>
                    #lhs: ::proto_hal::stasis::Entitled<#rhs>
                })
            },
        );

    constraints.extend(statewise_entitlements);

    Some(quote! { #(#constraints,)* })
}
