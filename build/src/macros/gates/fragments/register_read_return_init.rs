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
    register_path: &Path,
    register: &View<'cx, RegisterNode>,
    fields: &IndexMap<FieldKey, (Path, View<'cx, FieldNode>)>,
) -> TokenStream {
    let field_idents = fields.values().map(|(.., field)| field.ident());

    let field_values = fields.values().filter_map(|(field_path, field)| {
        read_value_expr(
            peripheral_path,
            register_path,
            field_path,
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
