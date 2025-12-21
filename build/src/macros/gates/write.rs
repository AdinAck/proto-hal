use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};
use model::Model;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, Ident};

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{
            binding_suggestions, module_suggestions, render_diagnostics, static_initial,
            unique_field_ident, validate_entitlements,
        },
    },
    parsing::{
        semantic::{
            self,
            policies::{self, field::RequireBinding},
        },
        syntax::Override,
    },
};

type Input<'cx> = semantic::Gate<'cx, policies::peripheral::ForbidPath, RequireBinding<'cx>>;

pub fn write(model: &Model, tokens: TokenStream) -> TokenStream {
    write_inner(model, tokens, false)
}
pub fn write_in_place(model: &Model, tokens: TokenStream) -> TokenStream {
    write_inner(model, tokens, true)
}

fn write_inner(model: &Model, tokens: TokenStream, in_place: bool) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input, model));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => diagnostics.push(
                syn::Error::new_spanned(
                    expr,
                    "stand-alone write access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(ident, format!("unexpected override \"{}\"", ident)).into(),
            ),
        };
    }

    let module_suggestions = module_suggestions(&args, &diagnostics);
    let binding_suggestions = binding_suggestions(&args, &diagnostics);
    let suggestions = quote! {
        #module_suggestions
        #binding_suggestions
    };
    let errors = render_diagnostics(diagnostics);

    let mut generics = Vec::new();
    let mut parameter_idents = Vec::new();
    let mut parameter_tys = Vec::new();
    let mut return_tys = Vec::new();
    let mut constraints = Vec::new();
    let mut addrs = Vec::new();
    let mut reg_write_values = Vec::new();
    let mut arguments = Vec::new();
    let mut conjures = Vec::new();
    let mut rebinds = Vec::new();

    for peripheral_item in input.visit_peripherals() {
        let peripheral_path = peripheral_item.path();

        for register_item in peripheral_item.registers().values() {
            let register_ident = register_item.ident();

            if register_item.fields().values().any(|field_item| {
                matches!(
                    field_item.entry(),
                    RequireBinding::DynamicTransition(..) | RequireBinding::Static(..)
                )
            }) {
                reg_write_values.push(fragments::register_write_value(
                    register_item,
                    static_initial(model, register_item).map(|value| value.get().to_token_stream()),
                    |register_item, field_item| {
                        let (input_generic, output_generic) =
                            fragments::generics(model, &input, register_item, field_item);

                        Some(match (field_item.entry(), input_generic, output_generic) {
                            (RequireBinding::DynamicTransition(..), ..) => {
                                let ident = unique_field_ident(
                                    register_item.peripheral(),
                                    register_item.register(),
                                    field_item.field(),
                                );

                                quote! { #ident.1 as u32 }
                            }
                            (RequireBinding::View(..), Some(generic), ..)
                            | (RequireBinding::Static(..), .., Some(generic)) => {
                                quote! { #generic::VALUE }
                            }
                            (
                                RequireBinding::Static(.., semantic::Transition::Expr(expr)),
                                ..,
                                None,
                            ) => quote! { #expr as u32 },
                            (..) => None?,
                        })
                    },
                ));
            }

            addrs.push(fragments::register_address(
                register_item.peripheral(),
                register_item.register(),
                &overridden_base_addrs,
            ));

            for field_item in register_item.fields().values() {
                let binding = field_item.entry().binding();
                if binding.is_ident() {
                    rebinds.push(binding.as_ref());
                }

                let (input_generic, output_generic) =
                    fragments::generics(model, &input, register_item, field_item);

                let input_ty = fragments::input_ty(
                    peripheral_path,
                    register_ident,
                    field_item.ident(),
                    field_item.field(),
                    input_generic.as_ref(),
                );

                let return_ty = fragments::transition_return_ty(
                    peripheral_path,
                    register_ident,
                    field_item.entry(),
                    field_item.field(),
                    field_item.ident(),
                    input_generic.as_ref(),
                    output_generic.as_ref(),
                );

                if let Some(local_constraints) = fragments::constraints(
                    &input,
                    model,
                    peripheral_path,
                    register_ident,
                    binding,
                    field_item.ident(),
                    field_item.field(),
                    input_generic.as_ref(),
                    output_generic.as_ref(),
                    &input_ty,
                    return_ty.as_ref(),
                ) {
                    constraints.push(local_constraints);
                }

                if let Some(return_ty) = return_ty {
                    return_tys.push(return_ty);
                    conjures.push(fragments::conjure());
                }

                if let Some(generic) = input_generic {
                    generics.push(generic);
                }

                if let Some(generic) = output_generic {
                    generics.push(generic);
                }

                parameter_idents.push(unique_field_ident(
                    register_item.peripheral(),
                    register_item.register(),
                    field_item.field(),
                ));

                let value_ty = field_item.field().access.get_write().map(|write| {
                    fragments::write_value_ty(
                        peripheral_path,
                        register_ident,
                        field_item.ident(),
                        write,
                    )
                });

                parameter_tys.push(fragments::write_parameter_ty(
                    binding,
                    &input_ty,
                    value_ty.as_ref(),
                ));

                arguments.push(fragments::write_argument(
                    peripheral_path,
                    register_ident,
                    field_item.ident(),
                    field_item.field(),
                    field_item.entry(),
                ));
            }
        }
    }

    let generics = (!generics.is_empty()).then_some(quote! {
        <#(#generics,)*>
    });

    let conjures = (!return_tys.is_empty()).then_some(quote! {
        unsafe {(
            #(
                #conjures
            ),*
        )}
    });

    let return_tys = (!return_tys.is_empty()).then_some(quote! {
        -> (#(#return_tys),*)
    });

    let constraints = (!constraints.is_empty()).then_some(quote! {
        where #(#constraints)*
    });

    let rebinds = in_place.then_some(quote! { let (#(#rebinds),*) = });
    let semicolon = in_place.then_some(quote! { ; });

    let unsafe_ = input
        .visit_fields()
        .any(|field| {
            let (peripheral, register) = field.field().parents();

            field.field().partial || register.partial || peripheral.partial
        })
        .then_some(quote! { unsafe });

    quote! {
        #rebinds {
            #suggestions
            #errors

            #unsafe_ fn gate #generics (#(#parameter_idents: #parameter_tys,)*) #return_tys #constraints {
                #(
                    unsafe {
                        ::core::ptr::write_volatile(
                            #addrs as *mut u32,
                            #reg_write_values
                        )
                    };
                )*

                #conjures
            }

            gate(#(#arguments,)*)
        } #semicolon
    }
}

fn validate<'cx>(input: &Input<'cx>, model: &'cx Model) -> Diagnostics {
    // Q: since transitions probe the model for write numericity, is this validation step necessary?

    // input
    //     .visit_fields()
    //     .filter_map(|field_item| {
    //         if !field_item.field().access.is_write() {
    //             Some(Diagnostic::field_must_be_writable(field_item.ident()))
    //         } else {
    //             None
    //         }
    //     })
    //     .collect()

    let mut diagnostics = Vec::new();

    // require non-inert fields
    for register_item in input.visit_registers() {
        let provided_fields = register_item.fields();

        // if no provided fields in this register perform a write, skip
        if provided_fields
            .values()
            .all(|field| field.entry().transition().is_none())
        {
            continue;
        }

        let mut concrete_missing_fields = IndexSet::new();
        let mut ambiguous_missing_fields = IndexSet::new();

        // for each bit
        for position in 0..32 {
            // if a provided field covers this bit, continue to check the next bit
            if provided_fields
                .values()
                .any(|field| field.field().domain().contains(&position))
            {
                continue;
            }

            // get all fields with domains containing this bit and no inert variant
            let positioned_missing_fields = register_item
                .register()
                .fields()
                .filter(|field| {
                    field.domain().contains(&position)
                        && field
                            .access
                            .get_write()
                            .is_some_and(|x| x.some_inert(model).is_none())
                })
                .map(|field| (field.module_name(), field))
                .collect::<IndexMap<_, _>>();

            // if there are no fields at this bit, continue to check the next bit
            if positioned_missing_fields.is_empty() {
                continue;
            }

            // if the fields on this bit overlap with a provided field, they must
            // be superpositioned and the provided field covers the overlapped
            // fields which need not be provided
            if positioned_missing_fields.values().any(|positioned| {
                provided_fields
                    .values()
                    .any(|provided| positioned.overlaps_with(provided.field()))
            }) {
                continue;
            }

            if positioned_missing_fields.len() == 1 {
                concrete_missing_fields
                    .insert(positioned_missing_fields.first().unwrap().0.clone());
            } else {
                ambiguous_missing_fields.extend(positioned_missing_fields.keys().cloned());
            }
        }

        if !concrete_missing_fields.is_empty() {
            if concrete_missing_fields.len() == 1 {
                diagnostics.push(Diagnostic::missing_concrete_field(
                    register_item.ident(),
                    &concrete_missing_fields[0],
                ));
            } else {
                diagnostics.push(Diagnostic::missing_concrete_fields(
                    register_item.ident(),
                    concrete_missing_fields.iter(),
                ));
            }
        }

        if !ambiguous_missing_fields.is_empty() {
            diagnostics.push(Diagnostic::missing_ambiguous_fields(
                register_item.ident(),
                ambiguous_missing_fields.iter(),
            ));
        }
    }

    validate_entitlements(input, model, &mut diagnostics);

    diagnostics
}
