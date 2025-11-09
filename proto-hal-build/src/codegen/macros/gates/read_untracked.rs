use std::collections::HashMap;

use ir::structures::hal::Hal;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{render_diagnostics, return_rank::ReturnRank, suggestions, unique_register_ident},
    },
    parsing::{
        semantic::{
            self,
            policies::{ForbidEntry, ForbidPeripherals},
        },
        syntax::Override,
    },
};

type EntryPolicy = ForbidEntry;
type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, EntryPolicy>;

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

    let return_rank = ReturnRank::from_input(&input, |_| true);
    let return_def = fragments::read_return_def(&return_rank);
    let return_ty = fragments::read_return_ty(&return_rank);
    let return_init = fragments::read_return_init(&return_rank);

    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();

    for register_item in input.visit_registers() {
        reg_idents.push(unique_register_ident(
            register_item.peripheral(),
            register_item.register(),
        ));
        addrs.push(fragments::register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));
    }

    let return_ty = return_ty.map(|return_ty| quote! { -> #return_ty });

    quote! {
        #suggestions
        #errors

        {
            #return_def

            unsafe fn gate() #return_ty {
                #(
                    let #reg_idents = unsafe {
                        ::core::ptr::read_volatile(#addrs as *const u32)
                    };
                )*

                #return_init
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
