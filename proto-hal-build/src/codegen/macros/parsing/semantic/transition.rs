use ir::structures::{
    field::{Field, Numericity},
    variant::Variant,
};
use syn::{Expr, Ident, LitInt};

use crate::codegen::macros::{diagnostic::Diagnostic, parsing::syntax};

pub enum Transition<'args, 'hal> {
    Variant(&'hal Variant),
    Expr(&'args Expr),
    Lit(&'args LitInt),
}

impl<'args, 'hal> Transition<'args, 'hal> {
    pub fn parse(
        transition: &'args syntax::Transition,
        field: &'hal Field,
        field_ident: &'args Ident,
    ) -> Result<Self, Diagnostic> {
        Ok(
            match (
                transition,
                &field
                    .access
                    .get_write()
                    .ok_or(Diagnostic::field_must_be_writable(field_ident))?
                    .numericity,
            ) {
                // a path is provided and the field is enumerated, so the transition tokens could be a variant
                // identifier
                (
                    syntax::Transition::Expr(expr @ Expr::Path(path)),
                    Numericity::Enumerated { variants },
                ) => {
                    if let Some(ident) = path.path.get_ident()
                        && let Some(variant) = variants
                            .values()
                            .find(|variant| &variant.type_name() == ident)
                    {
                        // a variant with the provided identifier was found
                        Self::Variant(variant)
                    } else {
                        // a variant with the provided identifier was not found, so the expr is preserved
                        Self::Expr(expr)
                    }
                }
                // the provided transition tokens could not be interpreted as a path, so they could not be the
                // identifier of a variant
                (syntax::Transition::Expr(expr), Numericity::Enumerated { .. }) => Self::Expr(expr),
                // the provided transition tokens are a non-literal expression, and the field is numeric so
                // the expr is preserved
                (syntax::Transition::Expr(expr), Numericity::Numeric) => Self::Expr(expr),
                // a literal transition value was provided, and the field is enumerated so a variant with a
                // corresponding bit value is searched for
                (syntax::Transition::Lit(lit_int), Numericity::Enumerated { variants }) => {
                    let bits = lit_int.base10_parse::<u32>()?;
                    let variant = variants
                        .values()
                        .find(|variant| variant.bits == bits)
                        .ok_or(Diagnostic::no_corresponding_variant(lit_int, field_ident))?;
                    Self::Variant(variant)
                }
                // a literal transition value was provided, and the field is numeric so the literal is preserved
                (syntax::Transition::Lit(lit_int), Numericity::Numeric) => Self::Lit(lit_int),
            },
        )
    }
}
