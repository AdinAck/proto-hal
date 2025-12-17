use indexmap::IndexSet;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::macros::{
    gates::{fragments::read_value_ty, utils::return_rank::ReturnRank},
    parsing::semantic::{FieldEntry, policies::Refine},
};

pub fn read_return_ty<'cx, EntryPolicy>(rank: &ReturnRank<'cx, EntryPolicy>) -> Option<TokenStream>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            register: register_item,
            field: field_item,
            ..
        } => Some(read_value_ty(
            &per & register_item.path(),
            field_item.ident(),
            field_item.field().access.get_read()?,
        )),
        ReturnRank::Register {
            register: register_item,
            ..
        } => {
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
