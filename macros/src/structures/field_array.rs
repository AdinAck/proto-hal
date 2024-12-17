use darling::FromMeta;
use syn::{ExprRange, Ident, Item};

use super::{field::FieldArgs, schema::SchemaSpec, Args};

#[derive(Debug, Clone, FromMeta)]
pub struct FieldArrayArgs {
    pub range: ExprRange,
    pub field: FieldArgs,
}

impl Args for FieldArrayArgs {
    const NAME: &str = "field_array";
}

#[derive(Debug)]
pub struct FieldArraySpec {
    pub ident: Ident,
    pub range: ExprRange,
    pub schema: SchemaSpec,
}

impl FieldArraySpec {
    pub fn parse<'a>(
        ident: Ident,
        args: FieldArrayArgs,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        todo!()
    }
}
