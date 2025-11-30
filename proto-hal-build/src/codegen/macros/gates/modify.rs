use std::collections::HashMap;

use model::structures::{
    entitlement::EntitlementIndex,
    field::{FieldNode, numericity::Numericity},
    model::{Model, View},
};
use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    gates::{
        fragments,
        utils::{
            mask, module_suggestions, render_diagnostics, return_rank::ReturnRank,
            scan_entitlements, static_initial, unique_field_ident, unique_register_ident,
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

pub fn modify(model: &Model, tokens: TokenStream) -> TokenStream {
    modify_inner(model, tokens, false)
}
pub fn modify_in_place(model: &Model, tokens: TokenStream) -> TokenStream {
    modify_inner(model, tokens, true)
}

fn modify_inner(model: &Model, tokens: TokenStream, in_place: bool) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input, model));

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

    let suggestions = module_suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let return_rank = ReturnRank::from_input(&input, |field_item| {
        let (RequireBinding::View(..) | RequireBinding::Dynamic(..)) = field_item.entry() else {
            return false;
        };

        field_is_unentangled(model, &input, field_item.field())
    });
    let return_ty = fragments::read_return_ty(&return_rank);
    let return_def = fragments::read_return_def(&return_rank);
    let return_init = fragments::read_return_init(&return_rank);
    let return_idents = match return_rank {
        ReturnRank::Empty => None,
        ReturnRank::Field { field_item, .. } => {
            Some(field_item.field().module_name().to_token_stream())
        }
        ReturnRank::Register { register_item, .. } => {
            Some(register_item.register().module_name().to_token_stream())
        }
        ReturnRank::Peripheral(map) => {
            let idents = map.keys();

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

    for register_item in input.visit_registers() {
        let register_path = register_item.path();
        let register_ident =
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
            read_reg_idents.push(register_ident.clone());
            read_addrs.push(addr.clone());
        }

        if register_item.fields().values().any(|field_item| {
            matches!(
                field_item.entry(),
                RequireBinding::Dynamic(..) | RequireBinding::Static(..)
            )
        }) {
            let static_initial = static_initial(model, register_item)
                .map(|value| value.get())
                .map(|static_initial| quote! { | #static_initial });
            let mask = mask(register_item.fields().values())
                .map(|value| !value.get())
                .map(|mask| quote! { & #mask });
            let initial = quote! {
                (#register_ident #mask) #static_initial
            };

            write_addrs.push(addr);
            reg_write_values.push(fragments::register_write_value(
                register_item,
                Some(initial),
                |r, f| {
                    let RequireBinding::Dynamic(..) = f.entry() else {
                        None?
                    };

                    let i = unique_field_ident(r.peripheral(), r.register(), f.field());

                    Some(quote! { #i(#return_idents) as u32 })
                },
            ));
        }

        for field_item in register_item.fields().values() {
            let binding = field_item.entry().binding();
            if binding.is_ident() {
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
                transition_return_tys.push(return_ty);
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

            arguments.push(fragments::modify_argument(
                &register_path,
                field_item.ident(),
                field_item.field(),
                field_item.entry(),
                return_idents.as_ref(),
            ));
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

    let return_y = return_ty.as_ref().map(|return_ty| quote! { , #return_ty });

    let return_x = return_idents
        .as_ref()
        .map(|return_idents| quote! { , #return_idents });

    let body = quote! {
        #cs

        #return_def

        fn gate #generics (#(#parameter_idents: #parameter_tys,)*) -> (#(#transition_return_tys),* #return_y) #constraints {
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

            unsafe { (#(#conjures),* #return_x) }
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

    // entitlements
    for field in input.visit_fields() {
        let (RequireBinding::Dynamic(..) | RequireBinding::Static(..)) = field.entry() else {
            continue;
        };

        // check for write entitlements
        if let Some(write_entitlements) =
            model.try_get_entitlements(EntitlementIndex::Write(*field.field().index()))
        {
            scan_entitlements(
                input,
                model,
                &mut diagnostics,
                field.ident(),
                write_entitlements,
            );
        }

        // check for statewise entitlements
        let Some(Numericity::Enumerated(enumerated)) = field.field().resolvable() else {
            continue;
        };

        for variant in enumerated.variants(model) {
            if let Some(statewise_entitlements) =
                model.try_get_entitlements(EntitlementIndex::Variant(*variant.index()))
            {
                scan_entitlements(
                    input,
                    model,
                    &mut diagnostics,
                    field.ident(),
                    statewise_entitlements,
                );
            }
        }
    }

    diagnostics
}

fn field_is_unentangled<'cx>(
    model: &'cx Model,
    input: &Input<'cx>,
    field: &View<'cx, FieldNode>,
) -> bool {
    for other_field_item in input.visit_fields() {
        let other_field_numericity = other_field_item.field().resolvable();

        for entitlement_set in other_field_item
            .field()
            .write_entitlements()
            .into_iter()
            .chain(
                other_field_item
                    .field()
                    .ontological_entitlements()
                    .into_iter(),
            )
            .chain(
                other_field_item
                    .field()
                    .hardware_write_entitlements()
                    .into_iter(),
            )
            .chain(
                other_field_numericity
                    .iter()
                    .flat_map(|numericity| numericity.variants(model))
                    .flatten()
                    .flat_map(|variant| variant.statewise_entitlements().into_iter()),
            )
        {
            for entitlement in *entitlement_set.as_ref() {
                if entitlement.field(model).index() == field.index() {
                    return false;
                }
            }
        }
    }

    true
}
