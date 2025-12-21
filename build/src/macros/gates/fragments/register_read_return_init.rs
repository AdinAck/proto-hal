use indexmap::IndexMap;
use model::{Peripheral, field::FieldNode, model::View, register::RegisterNode};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::macros::{gates::fragments::read_value_expr, parsing::semantic::FieldKey};

pub fn register_read_return_init<'cx>(
    peripheral_path: &Path,
    peripheral: &Peripheral,
    register_ty: &Ident,
    register: &View<'cx, RegisterNode>,
    fields: &IndexMap<FieldKey, View<'cx, FieldNode>>,
) -> TokenStream {
    let field_idents = fields.values().map(|field| field.module_name());

    let field_values = fields.values().filter_map(|field| {
        read_value_expr(
            peripheral_path,
            &register.ident,
            &field.ident,
            peripheral,
            register,
            field,
        )
    });

    quote! {
        #register_ty {
            #(
                #field_idents: #field_values,
            )*
        }
    }
}
