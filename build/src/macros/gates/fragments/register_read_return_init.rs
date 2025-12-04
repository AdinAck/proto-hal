use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::macros::{
    gates::fragments::read_value_expr,
    parsing::semantic::{FieldEntry, FieldItem, FieldKey, RegisterItem, policies::Refine},
};

pub fn register_read_return_init<'cx, EntryPolicy>(
    register_ident: Ident,
    register_item: &RegisterItem<'cx, EntryPolicy>,
    fields: &IndexMap<&FieldKey, &FieldItem<'cx, EntryPolicy>>,
) -> TokenStream
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
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
