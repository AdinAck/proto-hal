//! The model components related to diagnostics emitted while validating a [model](crate).

use diagnostic::{Class, Code, Diagnostic, Label};
use source::{Span, Spanned};

use crate::Node;

/// A diagnostic kind of the structure of a model.
///
/// *Note: The associated [`Code`] of the diagnostic Kind is derived from the enum discriminant.*
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Structure {
    /// A name given to more than one sibling.
    Duplicate = 1,
    /// A schema without variants.
    EmptySchema,
    /// A schema no field uses.
    UnusedSchema,
}

impl diagnostic::Kind for Structure {
    const CLASS: Class = Class::Structural;

    fn code(self) -> Code {
        Code::new(self as u16)
    }
}

/// "register `foo` is defined more than once".
pub fn duplicate(node: Node, name: &Spanned<String>, first: Span) -> Diagnostic {
    Diagnostic::error(
        Structure::Duplicate,
        format!("{node} `{}` is defined more than once", **name),
        Label::new("defined again here", name.span),
    )
    .supporting_label(Label::new("first defined here", first))
}

/// "schema `foo` has no variants".
pub fn empty_schema(name: Option<&Spanned<String>>, span: Span) -> Diagnostic {
    Diagnostic::error(
        Structure::EmptySchema,
        format!("{} has no variants", schema(name)),
        Label::new("declared here", name.map_or(span, |name| name.span)),
    )
}

/// "schema `foo` is never used".
pub fn unused_schema(name: Option<&Spanned<String>>, span: Span) -> Diagnostic {
    Diagnostic::warning(
        Structure::UnusedSchema,
        format!("{} is never used", schema(name)),
        Label::new("declared here", name.map_or(span, |name| name.span)),
    )
}

/// Produce a description of a schema for use in diagnostic messages i.e. "schema `foo`", or "schema" when it has no
/// name.
fn schema(name: Option<&Spanned<String>>) -> String {
    match name {
        Some(name) => format!("schema `{}`", **name),
        None => "schema".to_string(),
    }
}
