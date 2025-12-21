use proc_macro2::TokenStream;
use quote::quote;

use crate::macros::gates::{fragments::read_value_ty, utils::return_rank::ReturnRank};

pub fn read_return_ty<'cx>(rank: &ReturnRank<'cx>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            peripheral_path,
            register,
            field,
            ..
        } => Some(read_value_ty(
            peripheral_path,
            &register.ident,
            &field.ident,
            field.access.get_read()?,
        )),
        ReturnRank::Register { register, .. } => {
            let ident = register.type_name();

            Some(quote! { #ident })
        }
        ReturnRank::Peripheral(map) => {
            let idents = map
                .values()
                .map(|(_, peripheral, ..)| peripheral.type_name());

            Some(quote! { #(#idents),* })
        }
    }
}
