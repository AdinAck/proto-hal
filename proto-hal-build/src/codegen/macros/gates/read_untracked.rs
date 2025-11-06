use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};
use inflector::Inflector;
use ir::structures::hal::Hal;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments::{read_value_expr, read_value_ty, register_address},
        utils::{render_diagnostics, return_rank::ReturnRank, suggestions, unique_register_ident},
    },
    parsing::{
        semantic::{
            self, FieldItem, FieldKey, RegisterItem,
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

    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();

    for register_item in input.visit_registers() {
        reg_idents.push(unique_register_ident(
            register_item.peripheral(),
            register_item.register(),
        ));
        addrs.push(register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));
    }

    let return_rank = ReturnRank::from_input(&input, |_| true);
    let return_def = make_return_definition(&return_rank);
    let return_ty = make_return_ty(&return_rank).map(|return_ty| quote! { -> #return_ty });
    let return_init = make_return_init(&return_rank);

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

fn make_return_ty<'cx>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            register_item,
            field_item,
            ..
        } => Some(read_value_ty(
            &register_item.path(),
            field_item.ident(),
            &field_item.field().access.get_read()?.numericity,
        )),
        ReturnRank::Register { register_item, .. } => {
            let ident = register_item.register().type_name();

            Some(quote! { #ident })
        }
        ReturnRank::Peripheral(map) => {
            let idents = IndexSet::<Ident>::from_iter(
                map.values()
                    .flat_map(|registers| registers.values())
                    .map(|(register_item, ..)| register_item.peripheral().type_name()),
            )
            .into_iter();

            Some(quote! { #(#idents),* })
        }
    }
}

fn make_return_definition<'cx>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty | ReturnRank::Field { .. } => None,
        ReturnRank::Register {
            register_item,
            fields,
            ..
        } => Some(register_return_definition(
            register_item.register().type_name(),
            register_item,
            fields,
        )),
        ReturnRank::Peripheral(map) => {
            let defs = map
                .iter()
                .map(|(k, v)| {
                    (
                        Ident::new(k.to_string().to_pascal_case().as_str(), Span::call_site()),
                        v,
                    )
                })
                .map(|(ident, registers)| {
                    let register_idents = registers
                        .values()
                        .map(|(register_item, ..)| register_item.register().module_name());

                    let (register_tys, register_defs) = registers
                        .values()
                        .map(|(register_item, fields)| {
                            let ident = format_ident!(
                                "{}{}",
                                register_item.peripheral().type_name(),
                                register_item.register().type_name()
                            );

                            (
                                ident.clone(),
                                register_return_definition(ident, register_item, fields),
                            )
                        })
                        .collect::<(Vec<_>, Vec<_>)>();

                    quote! {
                        struct #ident {
                            #(#register_idents: #register_tys,)*
                        }

                        #(#register_defs)*
                    }
                });

            Some(quote! { #(#defs)* })
        }
    }
}

fn make_return_init<'cx>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            register_item,
            field_item,
            ..
        } => read_value_expr(
            &register_item.path(),
            field_item.ident(),
            register_item.peripheral(),
            register_item.register(),
            field_item.field(),
        ),
        ReturnRank::Register {
            register_item,
            fields,
            ..
        } => Some(register_return_init(
            register_item.register().type_name(),
            register_item,
            fields,
        )),
        ReturnRank::Peripheral(map) => {
            let values = map
                .iter()
                .map(|(k, v)| {
                    (
                        Ident::new(k.to_string().to_pascal_case().as_str(), Span::call_site()),
                        v,
                    )
                })
                .map(|(ident, registers)| {
                    let (register_idents, register_values) = registers
                        .values()
                        .map(|(register_item, fields)| {
                            (
                                register_item.register().module_name(),
                                register_return_init(
                                    format_ident!(
                                        "{}{}",
                                        register_item.peripheral().type_name(),
                                        register_item.register().type_name()
                                    ),
                                    register_item,
                                    fields,
                                ),
                            )
                        })
                        .collect::<(Vec<_>, Vec<_>)>();

                    quote! {
                        #ident {
                            #(#register_idents: #register_values,)*
                        }
                    }
                });

            Some(quote! { (#(#values),*) })
        }
    }
}

fn register_return_definition<'cx>(
    register_ident: Ident,
    register_item: &RegisterItem<'cx, EntryPolicy>,
    fields: &IndexMap<&FieldKey, &FieldItem<'cx, EntryPolicy>>,
) -> TokenStream {
    let field_idents = fields
        .values()
        .map(|field_item| field_item.field().module_name());

    let field_tys = fields.values().filter_map(|field_item| {
        Some(read_value_ty(
            &register_item.path(),
            field_item.ident(),
            &field_item.field().access.get_read()?.numericity,
        ))
    });

    quote! {
        struct #register_ident {
            #(
                #field_idents: #field_tys,
            )*
        }
    }
}

fn register_return_init<'cx>(
    register_ident: Ident,
    register_item: &RegisterItem<'cx, EntryPolicy>,
    fields: &IndexMap<&FieldKey, &FieldItem<'cx, EntryPolicy>>,
) -> TokenStream {
    let field_idents = fields
        .values()
        .map(|field_item| field_item.field().module_name());

    let field_values = fields.values().filter_map(|field_item| {
        read_value_expr(
            &register_item.path(),
            field_item.ident(),
            register_item.peripheral(),
            register_item.register(),
            field_item.field(),
        )
    });

    quote! {
        #register_ident {
            #(
                #field_idents: #field_values,
            )*
        }
    }
}
