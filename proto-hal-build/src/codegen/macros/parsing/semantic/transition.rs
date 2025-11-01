use ir::structures::{
    field::{Field, Numericity},
    variant::Variant,
};
use syn::{Expr, Ident, LitInt};

use crate::codegen::macros::{diagnostic::Diagnostic, parsing::syntax};

/// The semantic transition applied to a field.
///
/// If the field being transitioned has an enumerated numericity and the specific variant being transitioned to is
/// statically known, the corresponding variant element of the model will be the representation of the transition
/// destination. Otherwise, the parsed tokens (expression or literal) will be preserved and used instead.
pub enum Transition<'args, 'hal> {
    /// The transition destination is statically known to be this variant.
    Variant(&'hal Variant),
    /// The transition destination is an expression.
    Expr(&'args Expr),
    /// The transition destination is a literal integer.
    Lit(&'args LitInt),
}

impl<'args, 'hal> Transition<'args, 'hal> {
    /// Parse the transition input against the model to produce a semantic transition.
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
                // the provided transition tokens are a non-literal expression and the field is numeric, so
                // the expr is preserved
                (syntax::Transition::Expr(expr), Numericity::Numeric) => Self::Expr(expr),
                // a literal transition value was provided and the field is enumerated, so a variant with a
                // corresponding bit value is searched for
                (syntax::Transition::Lit(lit_int), Numericity::Enumerated { variants }) => {
                    let bits = lit_int.base10_parse::<u32>()?;
                    let variant = variants
                        .values()
                        .find(|variant| variant.bits == bits)
                        .ok_or(Diagnostic::no_corresponding_variant(lit_int, field_ident))?;
                    Self::Variant(variant)
                }
                // a literal transition value was provided and the field is numeric, so the literal is preserved
                (syntax::Transition::Lit(lit_int), Numericity::Numeric) => Self::Lit(lit_int),
            },
        )
    }
}
