use indexmap::IndexMap;
use model::{
    field::{FieldNode, numericity::Numericity},
    model::{Model, View},
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Ident, Path};

use crate::macros::{
    gates::fragments,
    parsing::{
        semantic::{self, policies},
        syntax::Binding,
    },
};

#[allow(clippy::too_many_arguments)]
pub fn constraints<'cx>(
    input: &semantic::Gate<
        'cx,
        policies::peripheral::ForbidPath,
        policies::field::RequireBinding<'cx>,
    >,
    model: &Model,
    peripheral_path: &Path,
    register_ident: &Ident,
    binding: &Binding,
    field_ident: &Ident,
    field: &View<'cx, FieldNode>,
    input_generic: Option<&Ident>,
    output_generic: Option<&Ident>,
    return_ty: Option<&TokenStream>,
) -> Option<TokenStream> {
    // if the subject field's write access has entitlements, the entitlements
    // must be satisfied in the input to the gate, and the fields used to
    // satisfy the entitlements cannot be written

    let mut constraints = Vec::new();
    let span = field_ident.span();

    if let Some(generic) = input_generic {
        constraints.push(
            quote_spanned! { span => #generic: ::proto_hal::stasis::State<#peripheral_path::#register_ident::#field_ident::Field> },
        );
    }

    if let Some(generic) = output_generic {
        constraints.push(
            quote_spanned! { span => #generic: ::proto_hal::stasis::Physical<#peripheral_path::#register_ident::#field_ident::Field> },
        );
    }

    if binding.is_mutated()
        && let Some(write_entitlements) = write_entitlements(input, field, span)
    {
        constraints.extend(write_entitlements);
    }

    if binding.is_viewed() || binding.is_dynamic() {
        return Some(quote_spanned! { span => #(#constraints,)* });
    }

    let Some(return_ty) = return_ty else {
        return Some(quote_spanned! { span => #(#constraints,)* });
    };

    if let Some(statewise_entitlements) =
        statewise_entitlements(input, model, field, return_ty, span)
    {
        constraints.extend(statewise_entitlements);
    }

    Some(quote! { #(#constraints,)* })
}

fn write_entitlements<'cx>(
    input: &semantic::Gate<
        'cx,
        policies::peripheral::ForbidPath,
        policies::field::RequireBinding<'cx>,
    >,
    field: &View<'cx, FieldNode>,
    span: Span,
) -> Option<Vec<TokenStream>> {
    let field_marker = {
        let (peripheral, register) = field.parents();

        let peripheral_path = input
            .get_peripheral(peripheral.module_name().to_string())?
            .path();
        let register_ident = register.module_name();
        let field_ident = field.module_name();

        quote! { #peripheral_path::#register_ident::#field_ident::Field }
    };

    // get entitlement *fields*
    let write_entitlements = field.write_entitlements()?;
    let entitlement_fields = write_entitlements.entitlement_fields();

    // render constraints for each field
    let constraints = entitlement_fields.filter_map(|entitlement_field| {
        // note: write entitlements can only be satisfied by input types
        // and the validation step is responsible for forbidding
        // transitioning of entitlement fields which are write access dependencies

        let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();

        let (entitlement_peripheral_item, entitlement_register_item, entitlement_field_item) = input.get_field(
            entitlement_peripheral.module_name().to_string(),
            entitlement_register.module_name().to_string(),
            entitlement_field.module_name().to_string(),
        )?;

        let generics = fragments::generics(
            entitlement_register_item,
            entitlement_field_item,
        );

        let entitlement_input_ty = fragments::input_ty(
            entitlement_peripheral_item.path(),
            entitlement_register_item.ident(),
            entitlement_field_item.ident(),
            entitlement_field_item.field(),
            generics.input.as_ref(),
        );

        Some(if let Some(write_pattern) = generics.write_pattern {
            quote_spanned! { span =>
                #field_marker: ::proto_hal::stasis::Entitled<#write_pattern, #entitlement_input_ty>
            }
        } else {
            quote_spanned! { span =>
                #field_marker: ::proto_hal::stasis::Entitled<::proto_hal::stasis::patterns::Fundamental<#field_marker, ::proto_hal::stasis::axes::Affordance>, #entitlement_input_ty>
            }
        })
    });

    Some(constraints.collect())
}

fn statewise_entitlements<'cx>(
    input: &semantic::Gate<
        'cx,
        policies::peripheral::ForbidPath,
        policies::field::RequireBinding<'cx>,
    >,
    model: &Model,
    field: &View<'cx, FieldNode>,
    return_ty: &TokenStream,
    span: Span,
) -> Option<Vec<TokenStream>> {
    // get entitlement *fields*
    let Numericity::Enumerated(enumerated) = field.resolvable()? else {
        None?
    };

    let statewise_entitlements = enumerated.variants(model).flat_map(|variant| {
        variant
            .statewise_entitlements()
            .into_iter()
            .flat_map(|x| x.entitlements())
    });

    let mut entitlement_fields = IndexMap::new();

    for entitlement in statewise_entitlements {
        let entitlement_field = entitlement.field(model);
        entitlement_fields.insert(*entitlement_field.index(), entitlement_field);
    }

    let constraints = entitlement_fields.values().filter_map(|entitlement_field| {
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

        let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();

        let (entitlement_peripheral_item, entitlement_register_item, entitlement_field_item) =
            input.get_field(
                entitlement_peripheral.module_name().to_string(),
                entitlement_register.module_name().to_string(),
                entitlement_field.module_name().to_string(),
            )?;

        let generics = fragments::generics(
            entitlement_register_item,
            entitlement_field_item,
        );

        let entitlement_return_ty = fragments::transition_return_ty(
            entitlement_peripheral_item.path(),
            entitlement_register_item.ident(),
            entitlement_field_item.entry(),
            entitlement_field_item.field(),
            entitlement_field_item.ident(),
            generics.input.as_ref(),
            generics.output.as_ref(),
        );

        let lhs = return_ty.clone();
        let rhs = entitlement_return_ty.unwrap_or(fragments::input_ty(
            entitlement_peripheral_item.path(),
            entitlement_register_item.ident(),
            entitlement_field_item.ident(),
            entitlement_field_item.field(),
            generics.input.as_ref(),
        ));

        Some(if let Some(statewise_pattern) = generics.statewise_pattern {
            quote_spanned! { span =>
                #lhs: ::proto_hal::stasis::Entitled<#statewise_pattern, #rhs>
            }
        } else {
            quote_spanned! { span =>
                #lhs: ::proto_hal::stasis::Entitled<::proto_hal::stasis::patterns::Fundamental<#lhs, ::proto_hal::stasis::axes::Statewise>, #rhs>
            }
        })
    });

    Some(constraints.collect())
}
