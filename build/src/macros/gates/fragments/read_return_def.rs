use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;

use crate::macros::gates::{fragments::register_read_return_def, utils::return_rank::ReturnRank};

pub fn read_return_def<'cx>(rank: &ReturnRank<'cx>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty | ReturnRank::Field { .. } => None,
        ReturnRank::Register {
            peripheral_path,
            register,
            fields,
            ..
        } => Some(register_read_return_def(
            peripheral_path,
            &register.type_name(),
            register,
            fields,
        )),
        ReturnRank::Peripheral(map) => {
            let defs = map
                .values()
                .map(|(peripheral_path, peripheral, registers)| {
                    let peripheral_ty = peripheral.type_name();
                    let register_idents = registers
                        .values()
                        .map(|(register, ..)| register.module_name());

                    let (register_tys, register_defs) = registers
                        .values()
                        .map(|(register, fields)| {
                            let ident =
                                format_ident!("{}{}", peripheral.type_name(), register.type_name());

                            (
                                ident.clone(),
                                register_read_return_def(peripheral_path, &ident, register, fields),
                            )
                        })
                        .collect::<(Vec<_>, Vec<_>)>();

                    quote! {
                        struct #peripheral_ty {
                            #(#register_idents: #register_tys,)*
                        }

                        #(#register_defs)*
                    }
                });

            Some(quote! { #(#defs)* })
        }
    }
}
