use indexmap::IndexMap;
use model::{field::FieldNode, model::View};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Path};

use crate::macros::{gates::fragments::read_value_ty, parsing::semantic::FieldKey};

pub fn register_read_return_def<'cx>(
    peripheral_path: &Path,
    regester_ty: &Ident,
    register_path: &Path,
    fields: &IndexMap<FieldKey, (Path, View<'cx, FieldNode>)>,
) -> TokenStream {
    let field_idents = fields.values().map(|(.., field)| field.ident());

    let field_tys = fields.values().filter_map(|(field_path, field)| {
        Some(read_value_ty(
            peripheral_path,
            register_path,
            field_path,
            field.access.get_read()?,
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
