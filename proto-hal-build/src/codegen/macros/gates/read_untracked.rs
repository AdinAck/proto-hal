use std::collections::HashMap;

use ir::structures::hal::Hal;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments::{read_value_expr, read_value_ty, register_address},
        utils::{render_diagnostics, suggestions, unique_register_ident},
    },
    parsing::{
        semantic::{
            self,
            policies::{ForbidEntry, ForbidPeripherals},
        },
        syntax::Override,
    },
};

type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, ForbidEntry>;

pub fn read_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
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
                    &expr,
                    "stand-alone read access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(&ident, format!("unexpected override \"{}\"", ident))
                    .into(),
            ),
        };
    }

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let mut returns = Vec::new();
    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();
    let mut read_values = Vec::new();

    for register_item in input.visit_registers() {
        let register_path = register_item.path();
        reg_idents.push(unique_register_ident(
            register_item.peripheral(),
            register_item.register(),
        ));
        addrs.push(register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            if let Some(read) = field_item.field().access.get_read() {
                returns.push(read_value_ty(
                    &register_path,
                    field_item.ident(),
                    &read.numericity,
                ));

                read_values.push(read_value_expr(
                    &register_path,
                    field_item.ident(),
                    register_item.peripheral(),
                    register_item.register(),
                    field_item.field(),
                ));
            }
        }
    }

    quote! {
        #suggestions
        #errors

        {
            unsafe fn gate() -> (#(#returns),*) {
                #(
                    let #reg_idents = unsafe {
                        ::core::ptr::read_volatile(#addrs as *const u32)
                    };
                )*

                (#(#read_values),*)
            }

            gate()
        }
    }
}

fn validate<'cx>(input: &Input<'cx>) -> Diagnostics {
    input
        .visit_fields()
        .filter_map(|field_item| {
            if !field_item.field().access.is_read() {
                Some(Diagnostic::field_must_be_readable(field_item.ident()))
            } else {
                None
            }
        })
        .collect()
}
