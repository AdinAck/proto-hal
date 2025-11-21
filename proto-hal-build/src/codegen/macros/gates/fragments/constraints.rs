use indexmap::IndexMap;
use model::structures::{
    field::{FieldNode, numericity::Numericity},
    model::{Model, View},
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Ident, Path};

use crate::codegen::macros::{
    gates::fragments,
    parsing::{
        semantic::{self, policies},
        syntax::Binding,
    },
};

pub fn constraints<'cx>(
    input: &semantic::Gate<
        'cx,
        policies::peripheral::ForbidPath,
        policies::field::RequireBinding<'cx>,
    >,
    model: &Model,
    register_path: &Path,
    binding: &Binding,
    field_ident: &Ident,
    field: &View<'cx, FieldNode>,
    input_generic: Option<&Ident>,
    output_generic: Option<&Ident>,
    input_ty: &TokenStream,
    return_ty: Option<&TokenStream>,
) -> Option<TokenStream> {
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

    if binding.is_mutated()
        && let Some(write_entitlements) = write_entitlements(input, field, input_ty, span)
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
    input_ty: &TokenStream,
    span: Span,
) -> Option<Vec<TokenStream>> {
    // get entitlement *fields*
    let write_entitlements = field.write_entitlements()?;
    let entitlement_fields = write_entitlements.entitlement_fields();

    // render constraints for each field
    let constraints = entitlement_fields.filter_map(|entitlement_field| {
        // note: write entitlements can only be satisfied by input types
        // and the validation step is responsible for forbidding
        // transitioning of entitlement fields which are write access dependencies

        let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();

        let (entitlement_register_item, entitlement_field_item) = input.get_field(
            entitlement_peripheral.module_name().to_string(),
            entitlement_register.module_name().to_string(),
            entitlement_field.module_name().to_string(),
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
            .flat_map(|x| x.iter())
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

        let (entitlement_register_item, entitlement_field_item) = input.get_field(
            entitlement_peripheral.module_name().to_string(),
            entitlement_register.module_name().to_string(),
            entitlement_field.module_name().to_string(),
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
    });

    Some(constraints.collect())
}
