use quote::{ToTokens as _, format_ident};
use syn::Ident;

use crate::macros::parsing::semantic::{self, FieldItem, RegisterItem, policies::field::GateEntry};

pub fn generics<'cx>(
    register_item: &RegisterItem<'cx, GateEntry<'cx>>,
    field_item: &FieldItem<'cx, GateEntry<'cx>>,
    is_dependency: bool,
) -> FieldGenerics {
    let generic_ident = format_ident!(
        "{}{}{}",
        register_item.peripheral().type_name(),
        register_item.register().type_name(),
        field_item.field().type_name(),
    );

    // there is an input generic iff the field is either an entitlement dependency of another, or
    // it is statically transitioned
    let input_generic = (is_dependency || matches!(field_item.entry(), GateEntry::Static(..)))
        .then_some(generic_ident.clone());

    // an output generic is only warrented if the transition destination is to be inferred
    let output_generic = if let GateEntry::Static(.., semantic::Transition::Expr(expr)) =
        field_item.entry()
        && expr.to_token_stream().to_string().trim() == "_"
    {
        Some(format_ident!("New{generic_ident}"))
    } else {
        None
    };

    // a write pattern generic is needed when the field is transitioning (being written to) *and* has write entitlements
    let write_pattern = (field_item.entry().transition().is_some()
        && field_item
            .field()
            .write_entitlements()
            .is_some_and(|space| !space.is_empty()))
    .then_some(format_ident!("{generic_ident}WritePattern"));

    // a statewise pattern generic is needed when the field is being statically transitioned and the output generic will
    // be the source of the statewise entitlement constraints
    let statewise_pattern = if let GateEntry::Static(..) = field_item.entry()
        && output_generic.is_some()
    {
        Some(format_ident!("{generic_ident}StatewisePattern"))
    } else {
        None
    };

    FieldGenerics {
        input: input_generic,
        output: output_generic,
        write_pattern,
        statewise_pattern,
    }
}

/// The generics associated with a field passed through the gate.
#[derive(Default)]
pub struct FieldGenerics {
    /// The generic for the state of the field upon *entering* the gate.
    pub input: Option<Ident>,
    /// The generic for the state of the field upon *exiting* the gate.
    pub output: Option<Ident>,
    /// The generic for the pattern used to satisfy the field's write entitlements.
    ///
    /// /// *Note: The write entitlements are required by and satisfy states immediately **before** a write to the incumbent
    /// register.*
    pub write_pattern: Option<Ident>,
    /// The generic for the pattern used to satisfy the field state's statewise entitlements.
    ///
    /// *Note: The statewise entitlements are required by and satisfy states immediately **after** a write to the incumbent
    /// register.*
    pub statewise_pattern: Option<Ident>,
}
