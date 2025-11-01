use ir::structures::{peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{Ident, LitInt, Path, spanned::Spanned};
use ters::ters;

#[derive(Debug)]
pub enum Kind {
    Syntax,
    UnexpectedRegister,
    ItemAlreadySpecified,
    ExpectedPeripheralPath,
    RegisterNotFound,
    FieldNotFound,
    PathCannotContinue,
    FieldMustBeWritable,
    NoCorrespondingVariant,
}

pub type Diagnostics = Vec<Diagnostic>;

#[ters]
#[derive(Debug)]
pub struct Diagnostic {
    #[get]
    kind: Kind,
    #[get]
    message: String,
    #[get]
    span: Span,
}

impl Diagnostic {
    fn new(kind: Kind, message: impl Into<String>, offending: &impl Spanned) -> Self {
        Self {
            kind,
            message: message.into(),
            span: offending.span(),
        }
    }

    /// unexpected register at end of path
    pub fn unexpected_register(register_ident: &Ident) -> Self {
        Self::new(
            Kind::UnexpectedRegister,
            "unexpected register at end of path",
            register_ident,
        )
    }

    /// this item has already been specified
    pub fn item_already_specified(item: &impl Spanned) -> Self {
        Self::new(
            Kind::ItemAlreadySpecified,
            format!("this item has already been specified"),
            item,
        )
    }

    /// expected path to peripheral
    pub fn expected_peripheral_path(offending: &impl Spanned) -> Self {
        Self::new(
            Kind::ExpectedPeripheralPath,
            "expected path to peripheral",
            offending,
        )
    }

    /// register "foo" not found in peripheral "bar"
    pub fn register_not_found(register_ident: &Ident, peripheral: &Peripheral) -> Self {
        Self::new(
            Kind::RegisterNotFound,
            format!(
                "register \"{register_ident}\" not found in peripheral \"{}\"",
                peripheral.module_name()
            ),
            register_ident,
        )
    }

    /// field "foo" not found in register "bar"
    pub fn field_not_found(field_ident: &Ident, register: &Register) -> Self {
        Self::new(
            Kind::FieldNotFound,
            format!(
                "field \"{field_ident}\" not found in register \"{}\"",
                register.module_name()
            ),
            field_ident,
        )
    }

    /// paths cannot continue after a field has been reached. reached field "foo"
    pub fn path_cannot_contine(path: &Path, field_ident: &Ident) -> Self {
        Self::new(
            Kind::PathCannotContinue,
            format!(
                "paths cannot continue after a field has been reached. reached field \"{field_ident}\"",
            ),
            path,
        )
    }

    /// field "foo" must be writable
    pub fn field_must_be_writable(field_ident: &Ident) -> Self {
        Self::new(
            Kind::FieldMustBeWritable,
            format!("field \"{field_ident}\" must be writable",),
            field_ident,
        )
    }

    /// literal "42" has no corresponding variant in field "foo"
    pub fn no_corresponding_variant(literal: &LitInt, field_ident: &Ident) -> Self {
        Self::new(
            Kind::NoCorrespondingVariant,
            format!("value \"{literal}\" has no corresponding variant in field \"{field_ident}\"",),
            literal,
        )
    }
}

impl From<Diagnostic> for Diagnostics {
    fn from(diagnostic: Diagnostic) -> Self {
        vec![diagnostic]
    }
}

impl From<Diagnostic> for syn::Error {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::new(diagnostic.span, diagnostic.message())
    }
}

impl From<syn::Error> for Diagnostic {
    fn from(err: syn::Error) -> Self {
        Self {
            kind: Kind::Syntax,
            message: err.to_string(),
            span: err.span(),
        }
    }
}
