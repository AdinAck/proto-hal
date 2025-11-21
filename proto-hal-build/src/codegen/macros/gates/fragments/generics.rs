use quote::{ToTokens as _, format_ident};
use syn::Ident;

use crate::codegen::macros::parsing::semantic::{
    self, FieldItem, RegisterItem, policies::field::RequireBinding,
};

pub fn generics<'cx>(
    register_item: &RegisterItem<'cx, RequireBinding<'cx>>,
    field_item: &FieldItem<'cx, RequireBinding<'cx>>,
) -> (Option<Ident>, Option<Ident>) {
    let input_generic = format_ident!(
        "{}{}{}",
        register_item.peripheral().type_name(),
        register_item.register().type_name(),
        field_item.field().type_name(),
    );

    match field_item.entry() {
        RequireBinding::View(..) => (Some(input_generic), None),
        RequireBinding::Dynamic(..) => (None, None),
        RequireBinding::Static(.., transition) => (
            Some(input_generic.clone()),
            if let semantic::Transition::Expr(expr) = transition
                && expr.to_token_stream().to_string().trim() == "_"
            {
                Some(format_ident!("New{input_generic}"))
            } else {
                None
            },
        ),
    }
}
