use std::collections::HashMap;

use model::structures::model::Model;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    gates::{
        fragments,
        utils::{mask, render_diagnostics, suggestions, unique_field_ident},
    },
    parsing::{
        semantic::{self, policies},
        syntax::Override,
    },
};

enum Scheme {
    FromZero,
    FromReset,
}

type Input<'cx> =
    semantic::Gate<'cx, policies::peripheral::ForbidPath, policies::field::TransitionOnly<'cx>>;
type RegisterItem<'cx> = semantic::RegisterItem<'cx, policies::field::TransitionOnly<'cx>>;

pub fn write_from_zero_untracked(model: &Model, tokens: TokenStream) -> TokenStream {
    write_untracked(Scheme::FromZero, model, tokens)
}

pub fn write_from_reset_untracked(model: &Model, tokens: TokenStream) -> TokenStream {
    write_untracked(Scheme::FromReset, model, tokens)
}

fn write_untracked(scheme: Scheme, model: &Model, tokens: TokenStream) -> TokenStream {
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

    let mut parameter_idents = Vec::new();
    let mut parameter_tys = Vec::new();
    let mut addrs = Vec::new();
    let mut parameter_write_values = Vec::new();
    let mut reg_write_values = Vec::new();

    for register_item in input.visit_registers() {
        let register_path = register_item.path();

        reg_write_values.push(reg_write_value(&scheme, register_item));

        addrs.push(fragments::register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            if let Some(write) = field_item.field().access.get_write() {
                parameter_idents.push(unique_field_ident(
                    register_item.peripheral(),
                    register_item.register(),
                    field_item.field(),
                ));

                parameter_tys.push(fragments::write_value_ty(
                    &register_path,
                    field_item.ident(),
                    write,
                ));

                parameter_write_values.push(fragments::write_argument_value(
                    &register_path,
                    field_item.ident(),
                    field_item.field(),
                    field_item.entry(),
                ));
            }
        }
    }

    quote! {
        #suggestions
        #errors

        {
            unsafe fn gate(#(#parameter_idents: #parameter_tys),*) {
                #(
                    unsafe {
                        ::core::ptr::write_volatile(
                            #addrs as *mut u32,
                            #reg_write_values
                        )
                    };
                )*
            }

            gate(#(#parameter_write_values),*)
        }
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

fn reg_write_value<'cx>(scheme: &Scheme, register_item: &RegisterItem<'cx>) -> TokenStream {
    let initial = match scheme {
        Scheme::FromZero => 0,
        Scheme::FromReset => {
            let mask = mask(register_item.fields().values());

            register_item.register().reset.unwrap_or(0)
                & !mask.map(|value| value.get()).unwrap_or(0)
        }
    };

    let values = register_item.fields().values().map(|field_item| {
        let field = field_item.field();
        let ident = unique_field_ident(register_item.peripheral(), register_item.register(), field);
        let shift = fragments::shift(field.offset);

        quote! { #ident as u32 #shift }
    });

    quote! {
        #initial #(| (#values) )*
    }
}
