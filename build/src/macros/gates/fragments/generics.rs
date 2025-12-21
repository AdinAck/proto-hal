use model::Model;
use quote::{ToTokens as _, format_ident};
use syn::Ident;

use crate::macros::{
    gates::utils::field_is_entangled,
    parsing::semantic::{
        self, FieldEntry, FieldItem, RegisterItem,
        policies::{self, Refine, field::RequireBinding},
    },
};

pub fn generics<'cx, EntryPolicy>(
    model: &'cx Model,
    input: &semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy>,
    register_item: &RegisterItem<'cx, RequireBinding<'cx>>,
    field_item: &FieldItem<'cx, RequireBinding<'cx>>,
) -> (Option<Ident>, Option<Ident>)
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let input_generic = format_ident!(
        "{}{}{}",
        register_item.peripheral().type_name(),
        register_item.register().type_name(),
        field_item.field().type_name(),
    );

    match field_item.entry() {
        RequireBinding::View(..) if field_is_entangled(model, input, field_item.field()) => {
            (Some(input_generic), None)
        }
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
        RequireBinding::Consumed(..) => (Some(input_generic), None),
        _ => (None, None),
    }
}
