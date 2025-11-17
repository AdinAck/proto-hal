use std::{collections::HashMap, ops::Deref};

use ir::structures::hal::Hal;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{
            mask, render_diagnostics, return_rank::ReturnRank, suggestions, unique_field_ident,
            unique_register_ident,
        },
    },
    parsing::{
        semantic::{
            self,
            policies::{ForbidPeripherals, PermitTransition},
        },
        syntax::Override,
    },
};

type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, PermitTransition<'cx>>;
type RegisterItem<'cx> = semantic::RegisterItem<'cx, PermitTransition<'cx>>;

pub fn modify_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
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

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let return_rank =
        ReturnRank::from_input(&input, |field_item| field_item.field().access.is_read());
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

    let mut closure_idents = Vec::new();
    let mut closure_return_tys = Vec::new();
    let mut read_reg_idents = Vec::new();
    let mut read_addrs = Vec::new();
    let mut write_addrs = Vec::new();
    let mut write_exprs = Vec::new();
    let mut reg_write_values = Vec::new();

    for register_item in input.visit_registers() {
        let register_path = register_item.path();
        let register_ident =
            unique_register_ident(register_item.peripheral(), register_item.register());
        let addr = fragments::register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        );

        read_reg_idents.push(register_ident);
        read_addrs.push(addr.clone());

        if register_item
            .fields()
            .values()
            .any(|field_item| field_item.entry().is_some())
        {
            write_addrs.push(addr);
            reg_write_values.push(reg_write_value(register_item, return_idents.as_ref()));
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
                    &register_path,
                    field_item.ident(),
                    write,
                ));

                write_exprs.push(fragments::write_argument_value(
                    &register_path,
                    field_item.ident(),
                    field_item.field(),
                    transition,
                ));
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

fn reg_write_value<'cx>(
    register_item: &RegisterItem<'cx>,
    return_idents: Option<&TokenStream>,
) -> TokenStream {
    let ident = unique_register_ident(register_item.peripheral(), register_item.register());
    let mask = mask(register_item.fields().values());

    let values = register_item.fields().values().map(|field_item| {
        let field = field_item.field();
        let ident = unique_field_ident(register_item.peripheral(), register_item.register(), field);
        let shift = fragments::shift(field.offset);

        quote! { #ident(#return_idents) as u32 #shift }
    });

    quote! {
        #ident & !#mask #(| (#values) )*
    }
}
