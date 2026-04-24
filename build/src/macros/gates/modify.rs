use std::collections::HashMap;

use model::{Model, field::FieldIndex};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{Expr, Ident};

use crate::macros::{
    diagnostic::Diagnostics,
    gates::{
        fragments::{self, FieldGenerics},
        utils::{
            self, binding_suggestions, field_is_dependency, mask, module_suggestions,
            render_diagnostics, return_rank::ReturnRank, static_initial, unique_field_ident,
            unique_register_ident, validate_entitlements,
        },
    },
    parsing::{
        semantic::{
            self,
            policies::{self, field::GateEntry},
        },
        syntax::Override,
    },
};

type Input<'cx> = semantic::Gate<'cx, policies::peripheral::ForbidPath, GateEntry<'cx>>;

pub fn modify(model: Model, tokens: TokenStream) -> TokenStream {
    modify_inner(model, tokens, false)
}
pub fn modify_in_place(model: Model, tokens: TokenStream) -> TokenStream {
    modify_inner(model, tokens, true)
}

fn modify_inner(model: Model, tokens: TokenStream, in_place: bool) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, &model);

    let field_dependencies =
        HashMap::<&FieldIndex, bool>::from_iter(input.visit_fields().map(|field| {
            (
                field.field().index(),
                field_is_dependency(&model, &input, field.field()),
            )
        }));

    diagnostics.extend(validate(&input, &model));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();
    let mut cs = None;

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => {
                cs.replace(quote! { #expr; });
            }
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

    let return_rank = ReturnRank::from_input_strict(&input, |field_item| {
        let (GateEntry::View(..) | GateEntry::Dynamic(..)) = field_item.entry() else {
            return false;
        };

        !field_dependencies.get(field_item.field().index()).unwrap()
    });
    let return_ty = fragments::read_return_ty(&return_rank);
    let return_def = fragments::read_return_def(&return_rank);
    let return_init = fragments::read_return_init(&return_rank);
    let return_idents = match return_rank {
        ReturnRank::Empty => None,
        ReturnRank::Field { field, .. } => Some(field.module_name().to_token_stream()),
        ReturnRank::Register { register, .. } => Some(register.module_name().to_token_stream()),
        ReturnRank::Peripheral(map) => {
            let idents = map
                .values()
                .map(|(_, peripheral, ..)| peripheral.module_name());

            Some(quote! { #(#idents),* })
        }
    };

    let mut generics = Vec::new();
    let mut parameter_idents = Vec::new();
    let mut parameter_tys = Vec::new();
    let mut transition_return_tys = Vec::new();
    let mut constraints = Vec::new();
    let mut read_reg_idents = Vec::new();
    let mut read_addrs = Vec::new();
    let mut write_addrs = Vec::new();
    let mut reg_write_values = Vec::new();
    let mut arguments = Vec::new();
    let mut conjures = Vec::new();
    let mut rebinds = Vec::new();

    // start with all fields in their input state
    let mut field_states = utils::input_field_states(&input, &field_dependencies);

    for peripheral_item in input.visit_peripherals() {
        let peripheral_path = peripheral_item.path();

        for register_item in peripheral_item.registers().values() {
            let register_ident = register_item.ident();
            let register_unique_ident =
                unique_register_ident(register_item.peripheral(), register_item.register());
            let addr = fragments::register_address(
                register_item.peripheral(),
                register_item.register(),
                &overridden_base_addrs,
            );

            if register_item
                .register()
                .fields()
                .any(|field| field.access.is_read())
            {
                read_reg_idents.push(register_unique_ident.clone());
                read_addrs.push(addr.clone());
            }

            if register_item.fields().values().any(|field_item| {
                matches!(
                    field_item.entry(),
                    GateEntry::DynamicTransition(..) | GateEntry::Static(..)
                )
            }) {
                let static_initial = static_initial(&model, register_item)
                    .map(|value| value.get())
                    .map(|static_initial| quote! { | #static_initial });
                let mask = mask(
                    register_item
                        .fields()
                        .values()
                        .filter(|field| field.entry().transition().is_some()),
                )
                .map(|value| !value.get())
                .map(|mask| quote! { & #mask });
                let initial = register_item
                    .register()
                    .fields()
                    .any(|field| field.access.is_read())
                    .then_some(quote! {
                        (#register_unique_ident #mask) #static_initial
                    });

                write_addrs.push(addr);
                reg_write_values.push(fragments::register_write_value(
                    register_item,
                    initial,
                    |r, f| {
                        let generics = fragments::generics(
                            r,
                            f,
                            *field_dependencies.get(f.field().index()).unwrap(),
                        );

                        match (f.entry(), generics) {
                            (GateEntry::DynamicTransition(..), ..) => {
                                let i = unique_field_ident(r.peripheral(), r.register(), f.field());

                                Some(quote! { (#i.1)(#return_idents) as u32 })
                            }
                            // WARN: take this with a grain of salt. at the time of writing, i don't entirely have a
                            // hold on how proto-hal works
                            (
                                GateEntry::Static(.., semantic::Transition::Expr(..)),
                                FieldGenerics {
                                    output: Some(output),
                                    ..
                                },
                            ) => Some(quote! { #output::VALUE }),
                            (GateEntry::Static(.., semantic::Transition::Expr(expr)), ..) => {
                                Some(quote! { #expr as u32 })
                            }
                            _ => None,
                        }
                    },
                ));
            }

            let post_field_states = utils::field_states_after_register(
                &field_states,
                &field_dependencies,
                peripheral_path,
                register_item,
            );

            for field_item in register_item.fields().values() {
                let binding = field_item.entry().binding();
                if binding.is_ident() {
                    rebinds.push(binding.as_ref());
                }

                let field_generics = fragments::generics(
                    register_item,
                    field_item,
                    *field_dependencies.get(field_item.field().index()).unwrap(),
                );

                let input_ty = fragments::input_ty(
                    peripheral_path,
                    register_item.ident(),
                    field_item.ident(),
                    field_item.field(),
                    field_generics.input.as_ref(),
                );

                let transition_return_ty = fragments::transition_return_ty(
                    peripheral_path,
                    register_item.ident(),
                    field_item.entry(),
                    field_item.field(),
                    field_item.ident(),
                    field_generics.output.as_ref(),
                );

                if let Some(local_constraints) = fragments::constraints(
                    &input,
                    peripheral_path,
                    register_ident,
                    binding,
                    field_item.ident(),
                    field_item.field(),
                    field_generics.input.as_ref(),
                    field_generics.output.as_ref(),
                    transition_return_ty.as_ref(),
                    &field_states,
                    &post_field_states,
                ) {
                    constraints.push(local_constraints);
                }

                if let Some(transition_return_ty) = &transition_return_ty {
                    transition_return_tys.push(transition_return_ty.clone());
                    conjures.push(fragments::conjure());
                }

                if let Some(generic) = field_generics.input {
                    generics.push(generic);
                }

                if let Some(generic) = field_generics.output {
                    generics.push(generic);
                }

                parameter_idents.push(unique_field_ident(
                    register_item.peripheral(),
                    register_item.register(),
                    field_item.field(),
                ));

                let write_value_ty = if field_item.entry().transition().is_some() {
                    field_item.field().access.get_write().map(|write| {
                        fragments::write_value_ty(
                            peripheral_path,
                            register_item.ident(),
                            field_item.ident(),
                            write,
                        )
                    })
                } else {
                    None
                };

                parameter_tys.push(fragments::modify_parameter_ty(
                    binding,
                    &input_ty,
                    write_value_ty.as_ref(),
                    return_ty.as_ref(),
                ));

                arguments.push(fragments::modify_argument(
                    peripheral_path,
                    register_item.ident(),
                    field_item.ident(),
                    field_item.field(),
                    field_item.entry(),
                    return_idents.as_ref(),
                ));
            }

            // the pre-states of the next register are the post-states of the current register
            field_states = post_field_states;
        }
    }

    let generics = (!generics.is_empty()).then_some(quote! {
        <#(#generics,)*>
    });

    let constraints = (!constraints.is_empty()).then_some(quote! {
        where #(#constraints)*
    });

    let rebinds = in_place.then_some(quote! { let (#(#rebinds),*) = });
    let semicolon = in_place.then_some(quote! { ; });

    let return_binding = return_idents
        .as_ref()
        .map(|return_idents| quote! { let (#return_idents) = #return_init; });

    let return_tys = {
        let tys = transition_return_tys.iter().chain(return_ty.iter());

        quote! { #(#tys),* }
    };

    let body_returns = {
        let items = conjures.iter().chain(return_idents.iter());

        quote! { #(#items),* }
    };

    let unsafe_ = input
        .visit_fields()
        .any(|field| {
            let (peripheral, register) = field.field().parents();

            field.field().leaky || register.leaky || peripheral.leaky
        })
        .then_some(quote! { unsafe });

    let body = quote! {
        #cs

        #return_def

        #unsafe_ fn gate #generics (#(#parameter_idents: #parameter_tys,)*) -> (#return_tys) #constraints {
            #(
                let #read_reg_idents = unsafe {
                    ::core::ptr::read_volatile(#read_addrs as *const u32)
                };
            )*

            #return_binding

            #(
                unsafe {
                    ::core::ptr::write_volatile(
                        #write_addrs as *mut u32,
                        #reg_write_values
                    )
                };
            )*

            unsafe { (#body_returns) }
        }

        gate(#(#arguments),*)
    };

    let body = if cs.is_none() {
        quote! {

            ::proto_hal::critical_section::with(|_| {
                #suggestions
                #errors

                #body
            })
        }
    } else {
        quote! {
            #rebinds {
                #suggestions
                #errors

                #body
            } #semicolon
        }
    };

    quote! {
        #body
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

    validate_entitlements(input, model, &mut diagnostics);

    diagnostics
}
