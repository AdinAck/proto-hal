use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::codegen::macros::{
    gates::{
        fragments::{read_value_expr, register_return_init},
        utils::return_rank::ReturnRank,
    },
    parsing::semantic::{FieldEntryRefinementInput, policies::Refine},
};

pub fn return_init<'cx, EntryPolicy>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream>
where
    EntryPolicy: Refine<'cx, Input = FieldEntryRefinementInput<'cx>>,
{
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
