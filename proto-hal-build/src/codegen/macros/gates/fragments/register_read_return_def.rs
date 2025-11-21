use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::codegen::macros::{
    gates::fragments::read_value_ty,
    parsing::semantic::{FieldEntry, FieldItem, FieldKey, RegisterItem, policies::Refine},
};

pub fn register_read_return_def<'cx, EntryPolicy>(
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

    let field_tys = fields.values().filter_map(|field_item| {
        Some(read_value_ty(
            &register_item.path(),
            field_item.ident(),
            field_item.field().access.get_read()?,
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
