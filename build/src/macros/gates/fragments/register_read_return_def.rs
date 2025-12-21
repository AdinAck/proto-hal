use indexmap::IndexMap;
use model::{field::FieldNode, model::View, register::RegisterNode};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::macros::{gates::fragments::read_value_ty, parsing::semantic::FieldKey};

pub fn register_read_return_def<'cx>(
    peripheral_path: &Path,
    regester_ty: &Ident,
    register_item: &View<'cx, RegisterNode>,
    fields: &IndexMap<FieldKey, View<'cx, FieldNode>>,
) -> TokenStream {
    let field_idents = fields.values().map(|field_item| field_item.module_name());

    let field_tys = fields.values().filter_map(|field_item| {
        Some(read_value_ty(
            peripheral_path,
            &register_item.ident,
            &field_item.ident,
            field_item.access.get_read()?,
        ))
    });

    quote! {
        struct #regester_ty {
            #(
                #field_idents: #field_tys,
            )*
        }
    }
}
