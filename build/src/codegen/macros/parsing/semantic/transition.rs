use model::{
    Model,
    field::{FieldNode, numericity::Numericity},
    variant::Variant,
};
use proc_macro2::Span;
use syn::{Expr, Ident, LitInt, spanned::Spanned};

use crate::codegen::macros::{diagnostic::Diagnostic, parsing::syntax};

/// The semantic transition applied to a field.
///
/// If the field being transitioned has an enumerated numericity and the specific variant being transitioned to is
/// statically known, the corresponding variant element of the model will be the representation of the transition
/// destination. Otherwise, the parsed tokens (expression or literal) will be preserved and used instead.
pub enum Transition<'cx> {
    /// The transition destination is statically known to be this variant.
    Variant(&'cx syntax::Transition, &'cx Variant),
    /// The transition destination is an expression.
    Expr(&'cx Expr),
    /// The transition destination is a literal integer.
    Lit(&'cx LitInt),
}

impl<'cx> Transition<'cx> {
    /// Parse the transition input against the model to produce a semantic transition.
    pub fn parse(
        model: &'cx Model,
        transition: &'cx syntax::Transition,
        field: &'cx FieldNode,
        field_ident: &'cx Ident,
    ) -> Result<Self, Diagnostic> {
        Ok(
            match (
                transition,
                &field
                    .access
                    .get_write()
                    .ok_or(Diagnostic::field_must_be_writable(field_ident))?,
            ) {
                // a path is provided and the field is enumerated, so the transition tokens could be a variant
                // identifier
                (
                    transition @ syntax::Transition::Expr(expr @ Expr::Path(path)),
                    Numericity::Enumerated(enumerated),
                ) => {
                    if let Some(ident) = path.path.get_ident()
                        && let Some(variant) = enumerated
                            .variants(model)
                            .find(|variant| &variant.type_name() == ident)
                    {
                        // a variant with the provided identifier was found
                        Self::Variant(transition, *variant)
                    } else {
                        // a variant with the provided identifier was not found, so the expr is preserved
                        Self::Expr(expr)
                    }
                }
                // the provided transition tokens could not be interpreted as a path, so they could not be the
                // identifier of a variant
                (syntax::Transition::Expr(expr), Numericity::Enumerated(..)) => Self::Expr(expr),
                // the provided transition tokens are a non-literal expression and the field is numeric, so
                // the expr is preserved
                (syntax::Transition::Expr(expr), Numericity::Numeric(..)) => Self::Expr(expr),
                // a literal transition value was provided and the field is enumerated, so a variant with a
                // corresponding bit value is searched for
                (
                    transition @ syntax::Transition::Lit(lit_int),
                    Numericity::Enumerated(enumerated),
                ) => {
                    let bits = lit_int.base10_parse::<u32>()?;
                    let variant = enumerated
                        .variants(model)
                        .find(|variant| variant.bits == bits)
                        .ok_or(Diagnostic::no_corresponding_variant(lit_int, field_ident))?;
                    Self::Variant(transition, *variant)
                }
                // a literal transition value was provided and the field is numeric, so the literal is preserved
                (syntax::Transition::Lit(lit_int), Numericity::Numeric(..)) => Self::Lit(lit_int),
            },
        )
    }

    /// The span of the transition source tokens.
    pub fn span(&self) -> Span {
        match self {
            Transition::Variant(transition, ..) => transition.span(),
            Transition::Expr(expr) => expr.span(),
            Transition::Lit(lit_int) => lit_int.span(),
        }
    }
}
