use indexmap::IndexMap;
use model::{
    Model,
    entitlement::{Entitlement, Entitlements},
};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, format_ident, quote};
use syn::Ident;

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{module_suggestions, render_diagnostics, scan_entitlements, unique_field_ident},
    },
    parsing::semantic::{self, policies},
};

type Input<'cx> =
    semantic::Gate<'cx, policies::peripheral::ConsumeOnly<'cx>, policies::field::ConsumeOnly<'cx>>;
type RegisterItem<'cx> = semantic::RegisterItem<'cx, policies::field::ConsumeOnly<'cx>>;
type FieldItem<'cx> = semantic::FieldItem<'cx, policies::field::ConsumeOnly<'cx>>;

pub fn unmask(model: &Model, tokens: TokenStream) -> TokenStream {
    unmask_inner(model, tokens, false)
}

pub fn unmask_in_place(model: &Model, tokens: TokenStream) -> TokenStream {
    unmask_inner(model, tokens, true)
}

fn unmask_inner(model: &Model, tokens: TokenStream, in_place: bool) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input, model));

    if !args.overrides.is_empty() {
        // TODO: override spans should be fixed
        diagnostics.push(
            syn::Error::new(
                Span::call_site(),
                "overrides are not accepted by \"unmask\"",
            )
            .into(),
        );
    }

    let mut generics = Vec::new();
    let mut parameters = Vec::new();
    let mut return_tys = Vec::new();
    let mut constraints = Vec::new();
    let mut arguments = Vec::new();
    let mut conjures = Vec::new();
    let mut rebinds = Vec::new();

    for peripheral_item in input.visit_peripherals() {
        let peripheral_ident = peripheral_item.peripheral().module_name();
        let peripheral_path = peripheral_item.path();

        let Some(ontological_entitlements) =
            peripheral_item.peripheral().ontological_entitlements()
        else {
            continue;
        };

        // peripheral is ontologically entitled to some field(s)

        make_constraints(
            &input,
            model,
            &mut constraints,
            &quote! { #peripheral_path::Reset },
            *ontological_entitlements,
        );

        let binding = peripheral_item.entry();

        if binding.is_ident() {
            rebinds.push(binding.as_ref().as_ref());
        }
        parameters.push(quote! { #peripheral_ident: #peripheral_path::Masked });
        return_tys.push(quote! { #peripheral_path::Reset });
        arguments.push(binding.to_token_stream());
        conjures.push(fragments::conjure());
    }

    for register_item in input.visit_registers() {
        for field_item in register_item.fields().values() {
            let (field_module_path, field_ty_path) = field_paths(register_item, field_item);
            let unique_field_ident = unique_field_ident(
                register_item.peripheral(),
                register_item.register(),
                field_item.field(),
            );
            let binding = field_item.entry();

            let Some(ontological_entitlements) = field_item.field().ontological_entitlements()
            else {
                // must be an entitlement of another entry, freeze!

                let generic = make_generic(register_item, field_item);

                // TODO: return frozen for reclaimation and rebinding

                parameters.push(quote! { #unique_field_ident: #field_ty_path<#generic>});
                generics.push(generic);
                arguments.push(binding.to_token_stream());

                continue;
            };

            make_constraints(
                &input,
                model,
                &mut constraints,
                &quote! { #field_module_path::Field },
                *ontological_entitlements,
            );

            if binding.is_ident() {
                rebinds.push(binding.as_ref());
            }
            parameters.push(quote! { #unique_field_ident: #field_module_path::Masked });
            return_tys.push(quote! { #field_ty_path<::proto_hal::stasis::Dynamic> });
            arguments.push(binding.to_token_stream());
            conjures.push(fragments::conjure());
        }
    }

    let suggestions = module_suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let constraints = (!constraints.is_empty()).then_some(quote! {
        where #(#constraints,)*
    });

    let rebinds = in_place.then_some(quote! { let (#(#rebinds),*) = });
    let semicolon = in_place.then_some(quote! { ; });

    quote! {
        #rebinds {
            #suggestions
            #errors

            fn gate<#(#generics,)*>(#(#parameters,)*) -> (#(#return_tys),*) #constraints {
                unsafe { (#(#conjures),*) }
            }

            gate(#(#arguments,)*)
        } #semicolon
    }
}

fn validate<'cx>(input: &Input<'cx>, model: &'cx Model) -> Diagnostics {
    // 1. all entitled items must have their entitlements present
    // 2. all unentitled items must be entitled to by at least one other item

    let mut diagnostics = Diagnostics::new();
    let mut entitlement_fields = IndexMap::new();

    for peripheral_item in input.visit_peripherals() {
        let Some(ontological_entitlements) =
            peripheral_item.peripheral().ontological_entitlements()
        else {
            diagnostics.push(Diagnostic::cannot_unmask_fundamental(
                peripheral_item.ident(),
            ));

            continue;
        };

        entitlement_fields.extend(scan_entitlements(
            input,
            model,
            &mut diagnostics,
            peripheral_item.ident(),
            ontological_entitlements,
        ));
    }

    for field_item in input.visit_fields() {
        let Some(ontological_entitlements) = field_item.field().ontological_entitlements() else {
            continue;
        };

        entitlement_fields.extend(scan_entitlements(
            input,
            model,
            &mut diagnostics,
            field_item.ident(),
            ontological_entitlements,
        ));
    }

    for field_item in input
        .visit_fields()
        .filter(|field_item| field_item.field().ontological_entitlements().is_none())
    {
        if !entitlement_fields.contains_key(field_item.field().index()) {
            diagnostics.push(Diagnostic::unincumbent_field(field_item.ident()));
        }
    }

    diagnostics
}

fn make_constraints<'cx>(
    input: &'cx Input<'cx>,
    model: &'cx Model,
    constraints: &mut Vec<TokenStream>,
    constrained_ty: &TokenStream,
    ontological_entitlements: &Entitlements,
) {
    for ontological_entitlement in ontological_entitlements {
        let Some((entitlement_register_item, entitlement_field_item)) =
            get_entitlement_input_items(input, model, ontological_entitlement)
        else {
            continue;
        };

        let (.., field_ty_path) = field_paths(entitlement_register_item, entitlement_field_item);
        let generic = make_generic(entitlement_register_item, entitlement_field_item);

        constraints.push(quote! {
            #constrained_ty: ::proto_hal::stasis::Entitled<#field_ty_path<#generic>>
        });
    }
}

fn get_entitlement_input_items<'cx>(
    input: &'cx Input<'cx>,
    model: &'cx Model,
    ontological_entitlement: &'cx Entitlement,
) -> Option<(&'cx RegisterItem<'cx>, &'cx FieldItem<'cx>)> {
    let entitlement_field = ontological_entitlement.field(model);
    let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();

    input.get_field(
        entitlement_peripheral.module_name().to_string(),
        entitlement_register.module_name().to_string(),
        entitlement_field.module_name().to_string(),
    )
}

fn make_generic<'cx>(register_item: &RegisterItem<'cx>, field_item: &FieldItem<'cx>) -> Ident {
    format_ident!(
        "{}{}{}",
        register_item.peripheral().type_name(),
        register_item.register().type_name(),
        field_item.field().type_name()
    )
}

fn field_paths<'cx>(
    register_item: &RegisterItem<'cx>,
    field_item: &FieldItem<'cx>,
) -> (TokenStream, TokenStream) {
    let register_path = register_item.path();
    let field_ident = field_item.ident();
    let field_ty = field_item.field().type_name();

    (
        quote! { #register_path::#field_ident },
        quote! { #register_path::#field_ident::#field_ty },
    )
}
