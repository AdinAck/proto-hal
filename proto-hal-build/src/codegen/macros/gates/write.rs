use std::collections::HashMap;

use model::structures::{field::numericity::Numericity, model::Model};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    gates::{
        fragments,
        utils::{render_diagnostics, suggestions, unique_field_ident},
    },
    parsing::{
        semantic::{
            self,
            policies::{ForbidPeripherals, RequireBinding},
        },
        syntax::Override,
    },
};

type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, RequireBinding<'cx>>;
type RegisterItem<'cx> = semantic::RegisterItem<'cx, RequireBinding<'cx>>;

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
    diagnostics.extend(validate(&input));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => diagnostics.push(
                syn::Error::new_spanned(
                    expr,
                    "stand-alone read access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(ident, format!("unexpected override \"{}\"", ident)).into(),
            ),
        };
    }

    let suggestions = suggestions(&args, &diagnostics);
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

    for register_item in input.visit_registers() {
        let register_path = register_item.path();

        if register_item.fields().values().any(|field_item| {
            matches!(
                field_item.entry(),
                RequireBinding::Dynamic(..) | RequireBinding::Static(..)
            )
        }) {
            reg_write_values.push(reg_write_value(model, register_item));
        }

        addrs.push(fragments::register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            let binding = field_item.entry().binding();
            if binding.is_moved() {
                rebinds.push(binding.as_ref());
            }

            let (input_generic, output_generic) = fragments::generics(register_item, field_item);

            let input_ty = fragments::input_ty(
                &register_path,
                field_item.ident(),
                field_item.field(),
                input_generic.as_ref(),
            );

            let return_ty = fragments::transition_return_ty(
                &register_path,
                field_item.entry(),
                field_item.field(),
                field_item.ident(),
                output_generic.as_ref(),
            );

            if let Some(local_constraints) = fragments::constraints(
                &input,
                model,
                &register_path,
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

            let value_ty =
                field_item.field().access.get_write().map(|write| {
                    fragments::write_value_ty(&register_path, field_item.ident(), write)
                });

            parameter_tys.push(fragments::write_parameter_ty(
                binding,
                &input_ty,
                value_ty.as_ref(),
            ));

            arguments.push(fragments::write_argument(
                &register_path,
                field_item.ident(),
                field_item.field(),
                field_item.entry(),
            ));
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

    quote! {
        #rebinds {
            #suggestions
            #errors

            fn gate #generics (#(#parameter_idents: #parameter_tys,)*) #return_tys #constraints {
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

fn validate<'cx>(_input: &Input<'cx>) -> Diagnostics {
    // Q: since transitions probe the model for write numericity, is this validation step necessary?

    Diagnostics::new()

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
}

fn reg_write_value<'cx>(model: &'cx Model, register_item: &RegisterItem<'cx>) -> TokenStream {
    // start with inert field values (or zero)
    let initial = {
        let inert = register_item
            .register()
            .fields()
            .filter_map(|field| {
                let intert_variant = match field.access.get_write()? {
                    Numericity::Numeric(..) => None?,
                    Numericity::Enumerated(enumerated) => enumerated.some_inert(model)?,
                };

                Some((field, intert_variant))
            })
            .fold(0, |acc, (field, variant)| {
                acc | (variant.bits << field.offset)
            });

        // mask out values to be filled in by user
        let mask = register_item.fields().values().fold(0, |acc, field_item| {
            let field = field_item.field();

            acc | ((u32::MAX >> (32 - field.width)) << field.offset)
        });

        // fill in statically known values from fields being statically transitioned
        let statics = register_item
            .fields()
            .values()
            .flat_map(|field_item| {
                let bits = match field_item.entry() {
                    RequireBinding::View(..) | RequireBinding::Dynamic(..) => None?,
                    RequireBinding::Static(.., transition) => match transition {
                        semantic::Transition::Variant(.., variant) => variant.bits,
                        semantic::Transition::Expr(..) => None?,
                        semantic::Transition::Lit(lit_int) => lit_int
                            .base10_parse::<u32>()
                            .expect("lit int should be valid"),
                    },
                };

                Some(bits << field_item.field().offset)
            })
            .reduce(|acc, value| acc | value)
            .unwrap_or(0);

        (inert & !mask) | statics
    };

    let values = register_item
        .fields()
        .values()
        .filter_map(|field_item| {
            let field = field_item.field();
            let shift = fragments::shift(field.offset);

            let (input_generic, output_generic) = fragments::generics(register_item, field_item);

            Some(match (field_item.entry(), input_generic, output_generic) {
                (RequireBinding::Dynamic(..), ..) => {
                    let ident = unique_field_ident(
                        register_item.peripheral(),
                        register_item.register(),
                        &field,
                    );

                    quote! { (#ident.1 #shift) as u32 }
                }
                (RequireBinding::View(..), Some(generic), ..)
                | (RequireBinding::Static(..), .., Some(generic)) => {
                    quote! { #generic::VALUE #shift }
                }
                (..) => None?,
            })
        })
        .collect::<Vec<_>>();

    match (initial == 0, values.is_empty()) {
        (false, false) => quote! { #initial #(| #values)* },
        (true, false) => quote! { #(#values)|* },
        (.., true) => quote! { #initial },
    }
}
