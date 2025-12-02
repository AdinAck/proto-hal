use proc_macro2::TokenStream;
use quote::quote;

use crate::codegen::macros::{
    gates::fragments,
    parsing::semantic::{FieldEntry, FieldItem, RegisterItem, policies::Refine},
};

/// A register write value starting from an initial value, applying a runtime mask, and inserting
/// the provided field-value expressions.
pub fn register_write_value<'cx, EntryPolicy>(
    register_item: &RegisterItem<'cx, EntryPolicy>,
    initial: Option<TokenStream>,
    expr_factory: impl Fn(
        &RegisterItem<'cx, EntryPolicy>,
        &FieldItem<'cx, EntryPolicy>,
    ) -> Option<TokenStream>,
) -> TokenStream
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    let mut values = register_item
        .fields()
        .values()
        .filter_map(|field_item| {
            let field = field_item.field();
            let shift = fragments::shift(field.offset);

            let expr = expr_factory(register_item, field_item)?;

            Some(quote! { (#expr) #shift })
        })
        .peekable();

    match (initial, values.peek().is_some()) {
        (Some(initial), true) => {
            quote! { (#initial) #(| (#values) )* }
        }
        (Some(initial), false) => {
            quote! { (#initial) }
        }
        (None, true) => {
            quote! { #( (#values) )|* }
        }
        (None, false) => {
            // this value will never actually be used
            // as this only happens when the transition is
            // invalid
            quote! { 0 }
        }
    }
}
