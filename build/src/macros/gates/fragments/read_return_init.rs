use inflector::Inflector as _;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::Ident;

use crate::macros::{
    gates::{
        fragments::{read_value_expr, register_read_return_init},
        utils::return_rank::ReturnRank,
    },
    parsing::semantic::{FieldEntry, policies::Refine},
};

pub fn read_return_init<'cx, EntryPolicy>(
    rank: &ReturnRank<'cx, EntryPolicy>,
) -> Option<TokenStream>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            register: register_item,
            field: field_item,
            ..
        } => read_value_expr(
            &register_item.path(),
            field_item.ident(),
            register_item.peripheral(),
            register_item.register(),
            field_item.field(),
        ),
        ReturnRank::Register {
            register: register_item,
            fields,
            ..
        } => Some(register_read_return_init(
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
                                register_read_return_init(
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
