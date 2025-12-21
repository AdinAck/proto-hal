use std::{collections::HashMap, ops::Deref};

use model::Model;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, Ident};

use crate::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{
            mask, module_suggestions, render_diagnostics, return_rank::ReturnRank,
            unique_field_ident, unique_register_ident,
        },
    },
    parsing::{
        semantic::{self, policies},
        syntax::Override,
    },
};

type Input<'cx> =
    semantic::Gate<'cx, policies::peripheral::ForbidPath, policies::field::PermitTransition<'cx>>;

pub fn modify_untracked(model: &Model, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input));

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

    let return_rank = ReturnRank::from_input_relaxed(&input, |field| field.access.is_read());
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

    let mut closure_idents = Vec::new();
    let mut closure_return_tys = Vec::new();
    let mut read_reg_idents = Vec::new();
    let mut read_addrs = Vec::new();
    let mut write_addrs = Vec::new();
    let mut write_exprs = Vec::new();
    let mut reg_write_values = Vec::new();

    for peripheral_item in input.visit_peripherals() {
        let peripheral_path = peripheral_item.path();

        for register_item in peripheral_item.registers().values() {
            let register_unique_ident =
                unique_register_ident(register_item.peripheral(), register_item.register());
            let addr = fragments::register_address(
                peripheral_item.peripheral(),
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

            if register_item
                .fields()
                .values()
                .any(|field_item| field_item.entry().is_some())
            {
                let initial = &register_unique_ident;
                let mask = mask(register_item.fields().values()).map(|non_zero| {
                    let inverted = !non_zero.get();
                    quote! { & #inverted }
                });

                write_addrs.push(addr);
                reg_write_values.push(fragments::register_write_value(
                    register_item,
                    Some(quote! { #initial #mask }),
                    |r, f| {
                        let i = unique_field_ident(r.peripheral(), r.register(), f.field());

                        Some(quote! { #i(#return_idents) as u32 })
                    },
                ));
            }

            for field_item in register_item.fields().values() {
                if let Some(write) = field_item.field().access.get_write()
                    && let Some(transition) = field_item.entry().deref()
                {
                    closure_idents.push(unique_field_ident(
                        register_item.peripheral(),
                        register_item.register(),
                        field_item.field(),
                    ));

                    closure_return_tys.push(fragments::write_value_ty(
                        peripheral_path,
                        register_item.ident(),
                        field_item.ident(),
                        write,
                    ));

                    write_exprs.push(fragments::write_argument_value(
                        peripheral_path,
                        register_item.ident(),
                        field_item.ident(),
                        field_item.field(),
                        transition,
                    ));
                }
            }
        }
    }

    let return_ty_with_arrow = return_ty
        .as_ref()
        .map(|return_ty| quote! { -> (#return_ty) });

    let body = quote! {
        #cs

        #return_def

        unsafe fn gate(#(#closure_idents: impl FnOnce(#return_ty) -> #closure_return_tys,)*) #return_ty_with_arrow {
            #(
                let #read_reg_idents = unsafe {
                    ::core::ptr::read_volatile(#read_addrs as *const u32)
                };
            )*

            let (#return_idents) = #return_init;

            #(
                unsafe {
                    ::core::ptr::write_volatile(
                        #write_addrs as *mut u32,
                        #reg_write_values
                    )
                };
            )*

            (#return_idents)
        }

        gate(#(|#return_idents| { #write_exprs },)*)
    };

    let body = if cs.is_none() {
        quote! {
            ::proto_hal::critical_section::with(|_| {
                #body
            })
        }
    } else {
        quote! {{ #body }}
    };

    quote! {
        #suggestions
        #errors
        #body
    }
}

fn validate<'cx>(input: &Input<'cx>) -> Diagnostics {
    input
        .visit_fields()
        .filter_map(|field_item| {
            if !field_item.field().access.is_read() && field_item.entry().is_none() {
                Some(Diagnostic::field_must_be_readable(field_item.ident()))
            } else {
                None
            }
        })
        .collect()
}
