use std::collections::HashMap;

use model::{Model, field::access::Access};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments,
        utils::{
            module_suggestions, render_diagnostics, return_rank::ReturnRank, unique_field_ident,
            unique_register_ident,
        },
    },
    parsing::{
        semantic::{self, FieldItem, RegisterItem, policies},
        syntax::Override,
    },
};

type EntryPolicy<'cx> = policies::field::BindingOnly<'cx>;
type Input<'cx> = semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy<'cx>>;

pub fn read(model: &Model, tokens: TokenStream) -> TokenStream {
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

    let suggestions = module_suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let return_rank = ReturnRank::from_input(&input, |_| true);
    let return_def = fragments::read_return_def(&return_rank);
    let return_ty = fragments::read_return_ty(&return_rank);
    let return_init = fragments::read_return_init(&return_rank);

    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();
    let mut parameters = Vec::new();
    let mut bindings = Vec::new();

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

        for field_item in register_item.fields().values() {
            parameters.push(make_parameter(register_item, field_item));
            bindings.push(field_item.entry().as_ref());
        }
    }

    let return_ty = return_ty.map(|return_ty| quote! { -> #return_ty });

    quote! {
        #suggestions
        #errors

        {
            #return_def

            fn gate(#(#parameters,)*) #return_ty {
                #(
                    let #reg_idents = unsafe {
                        ::core::ptr::read_volatile(#addrs as *const u32)
                    };
                )*

                #return_init
            }

            gate(#(#bindings,)*)
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

fn make_parameter<'cx>(
    register_item: &RegisterItem<'cx, EntryPolicy<'cx>>,
    field_item: &FieldItem<'cx, EntryPolicy<'cx>>,
) -> TokenStream {
    let unique_ident = unique_field_ident(
        register_item.peripheral(),
        register_item.register(),
        field_item.field(),
    );
    let path = register_item.path();
    let ident = field_item.ident();
    let ty = field_item.field().type_name();

    let ref_ = if let Access::Store(..) = &field_item.field().access {
        quote! { & }
    } else {
        quote! { &mut }
    };

    quote! { #unique_ident: #ref_ #path::#ident::#ty<::proto_hal::stasis::Dynamic> }
}
