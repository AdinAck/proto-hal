use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::macros::gates::{
    fragments::{read_value_expr, register_read_return_init},
    utils::return_rank::ReturnRank,
};

pub fn read_return_init<'cx>(rank: &ReturnRank<'cx>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            peripheral_path,
            peripheral,
            register,
            field,
            ..
        } => read_value_expr(
            peripheral_path,
            &register.ident,
            &field.ident,
            peripheral,
            register,
            field,
        ),
        ReturnRank::Register {
            peripheral_path,
            peripheral,
            register,
            fields,
            ..
        } => Some(register_read_return_init(
            peripheral_path,
            peripheral,
            &register.type_name(),
            register,
            fields,
        )),
        ReturnRank::Peripheral(map) => {
            let values = map
                .values()
                .map(|(peripheral_path, peripheral, registers)| {
                    let peripheral_ty = peripheral.type_name();
                    let (register_idents, register_values) = registers
                        .values()
                        .map(|(register, fields)| {
                            (
                                register.module_name(),
                                register_read_return_init(
                                    peripheral_path,
                                    peripheral,
                                    &format_ident!(
                                        "{}{}",
                                        peripheral.type_name(),
                                        register.type_name()
                                    ),
                                    register,
                                    fields,
                                ),
                            )
                        })
                        .collect::<(Vec<_>, Vec<_>)>();

                    quote! {
                        #peripheral_ty {
                            #(#register_idents: #register_values,)*
                        }
                    }
                });

            Some(quote! { (#(#values),*) })
        }
    }
}
