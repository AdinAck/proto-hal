use std::{collections::HashMap, ops::Range};

use darling::FromMeta;
use syn::{ExprRange, Ident, Item};
use tiva::Validator;

use crate::utils::{parse_expr_range, FieldOffset, Spanned, SynErrorCombinator};

use super::{
    field::{Field, FieldArgs, FieldSpec},
    schema::Schema,
    Args,
};

#[derive(Debug, Clone, FromMeta)]
pub struct FieldArrayArgs {
    pub range: ExprRange,

    #[darling(flatten)]
    pub field: FieldArgs,
}

impl Args for FieldArrayArgs {
    const NAME: &str = "field_array";
}

#[derive(Debug)]
pub struct FieldArray {
    pub inherited: Field,
    pub range: Range<u32>,
}

impl FieldArray {
    pub fn parse<'a>(
        ident: Ident,
        offset: FieldOffset,
        schemas: &HashMap<Ident, Schema>,
        args: Spanned<FieldArrayArgs>,
        items: impl Iterator<Item = &'a Item>,
    ) -> syn::Result<Self> {
        // this does not actuall represent a field,
        // but merely the structure of a field
        let pseudo_field = Field::validate(FieldSpec::parse(
            ident,
            offset,
            schemas,
            args.field.clone().with_span(args.span()),
            items,
        )?)?;

        let range = parse_expr_range(&args.range)?;

        Ok(Self {
            inherited: pseudo_field,
            range,
        })
    }
}

impl FieldArray {
    pub fn count(&self) -> usize {
        self.range.clone().count() as _
    }

    pub fn to_fields(&self) -> syn::Result<Vec<Field>> {
        let mut errors = SynErrorCombinator::new();
        let mut fields = Vec::new();

        let inherited = &self.inherited;

        let mut offset = inherited.offset;

        let replace_pos = inherited
            .ident
            .to_string()
            .rfind("X")
            .ok_or(syn::Error::new(
                inherited.ident.span(),
                "field array module ident must contain an 'X' to indicate replacement location",
            ))?;

        // generate fields
        for i in self.range.clone() {
            let mut s = inherited.ident.to_string();
            s.replace_range(replace_pos..replace_pos + 1, &i.to_string());
            let ident = Ident::new(&s, inherited.ident.span());

            let args = inherited.args.clone();
            let access = inherited.access.clone();

            let get_field = || Field::validate(FieldSpec::new(args, ident, offset, access)?);

            errors.maybe_then(get_field(), |field| {
                offset += field.width();

                fields.push(field);
            });
        }

        errors.coalesce()?;

        Ok(fields)
    }
}
