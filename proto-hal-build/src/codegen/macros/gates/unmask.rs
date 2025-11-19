use indexmap::IndexSet;
use model::structures::model::Model;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote};

use crate::codegen::macros::{
    diagnostic::Diagnostic,
    gates::{
        fragments,
        utils::{render_diagnostics, suggestions, unique_field_ident},
    },
    parsing::semantic::{
        self,
        policies::{PermitPeripherals, RequireBinding},
    },
};

type Input<'cx> = semantic::Gate<'cx, PermitPeripherals, RequireBinding<'cx>>;
type RegisterItem<'cx> = semantic::RegisterItem<'cx, RequireBinding<'cx>>;

fn unmask_inner(model: &Model, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    // diagnostics.extend(validate(&input, model));

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

    let mut parameters = Vec::new();
    let mut return_tys = Vec::new();
    let mut arguments = Vec::new();
    let mut conjures = Vec::new();
    let mut transmutes = Vec::new();

    for peripheral_item in input.visit_peripherals() {
        let peripheral_ident = peripheral_item.ident();

        let Some(binding) = peripheral_item.binding() else {
            // TODO: shouldn't need to do this
            diagnostics.push(Diagnostic::expected_binding(peripheral_ident));
            continue;
        };

        let Some(ontological_entitlements) =
            peripheral_item.peripheral().ontological_entitlements()
        else {
            continue;
        };

        for ontological_entitlement in *ontological_entitlements {
            let Some((entitlement_register_item, entitlement_field_item)) =
                get_entitlement_input_items(model, &input, ontological_entitlement)
            else {
                continue;
            };

            let peripheral_path = peripheral_item.path();

            parameters.push(quote! { #peripheral_ident: #peripheral_path::Masked });
            return_tys.push(quote! { #peripheral_path::Reset });
            arguments.push(binding.to_token_stream());
            conjures.push(fragments::conjure());
        }
    }

    for register_item in input.visit_registers() {
        for field_item in register_item.fields().values() {
            let register_path = register_item.path();
            let field_ident = field_item.ident();
            let field_ty = field_item.field().type_name();
            let unique_field_ident = unique_field_ident(
                register_item.peripheral(),
                register_item.register(),
                field_item.field(),
            );

            let Some(ontological_entitlements) = field_item.field().ontological_entitlements()
            else {
                // must be an entitlement of another entry, freeze!

                parameters
                    .push(quote! { #unique_field_ident: #register_path::#field_ident::Masked });
                return_tys.push(quote! { #register_path::#field_ident::#field_ty<::proto_hal::stasis::Dynamic> });
                arguments.push(binding.to_token_stream());
                conjures.push(fragments::conjure());

                continue;
            };

            for ontological_entitlement in *ontological_entitlements {
                let Some((entitlement_register_item, entitlement_field_item)) =
                    get_entitlement_input_items(model, &input, ontological_entitlement)
                else {
                    continue;
                };

                let binding = field_item.entry().binding();

                parameters
                    .push(quote! { #unique_field_ident: #register_path::#field_ident::Masked });
                return_tys.push(quote! { #register_path::#field_ident::#field_ty<::proto_hal::stasis::Dynamic> });
                arguments.push(binding.to_token_stream());
                conjures.push(fragments::conjure());
            }
        }
    }

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    quote! {
        {
            #suggestions
            #errors

            fn gate(#(#parameters,)*) -> (#(#return_tys),*) {
                unsafe { (#(#conjures),*) }
            }

            gate(#(#arguments,)*)
        }
    }
}

fn get_entitlement_input_items<'cx>(
    model: &'cx Model,
    input: &'cx semantic::Gate<'cx, PermitPeripherals, RequireBinding<'cx>>,
    ontological_entitlement: &'cx model::structures::entitlement::Entitlement,
) -> Option<(
    &'cx semantic::RegisterItem<'cx, RequireBinding<'cx>>,
    &'cx semantic::FieldItem<'cx, RequireBinding<'cx>>,
)> {
    let entitlement_field = ontological_entitlement.field(model);
    let (entitlement_peripheral, entitlement_register) = entitlement_field.parents();

    input.get_field(
        entitlement_peripheral.module_name().to_string(),
        entitlement_register.module_name().to_string(),
        entitlement_field.module_name().to_string(),
    )
}
