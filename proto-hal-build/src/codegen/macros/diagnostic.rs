use model::structures::{peripheral::Peripheral, register::Register};
use proc_macro2::Span;
use syn::{Ident, LitInt, Path, spanned::Spanned};
use ters::ters;

use crate::codegen::macros::parsing::syntax::Binding;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    // parsing
    Erased = 0,
    UnexpectedRegister,
    UnexpectedPeripheral,
    ItemAlreadySpecified,
    ExpectedPeripheralPath,
    RegisterNotFound,
    FieldNotFound,
    PathCannotContinue,
    FieldMustBeReadable,
    FieldMustBeWritable,
    NoCorrespondingVariant,
    UnexpectedBinding,
    ExpectedBinding,
    BindingMustBeView,
    BindingCannotBeView,
    BindingCannotBeDynamic,
    UnexpectedTransition,
    ExpectedTransition,

    // validation
    MissingEntitlements = 1000,
    MissingFields,
    CannotUnmaskFundamental,
    UnincumbentField,
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

    /// unexpected peripheral at end of path
    pub fn unexpected_peripheral(offending: &impl Spanned) -> Self {
        Self::new(
            Kind::UnexpectedPeripheral,
            "unexpected peripheral at end of path",
            offending,
        )
    }

    /// this item has already been specified
    pub fn item_already_specified(item: &impl Spanned) -> Self {
        Self::new(
            Kind::ItemAlreadySpecified,
            "this item has already been specified",
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

    /// field "foo" must be readable
    pub fn field_must_be_readable(field_ident: &Ident) -> Self {
        Self::new(
            Kind::FieldMustBeReadable,
            format!("field \"{field_ident}\" must be readable",),
            field_ident,
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

    /// unexpected binding
    pub fn unexpected_binding(binding: &Binding) -> Self {
        Self::new(
            Kind::UnexpectedBinding,
            "unexpected binding",
            binding.as_ref(),
        )
    }

    /// expected binding
    pub fn expected_binding(offending: &impl Spanned) -> Self {
        Self::new(Kind::ExpectedBinding, "expected binding", offending)
    }

    /// binding must be a view (&foo)
    pub fn binding_must_be_view(binding: &Binding) -> Self {
        Self::new(
            Kind::BindingMustBeView,
            "binding must be a view (&foo)",
            binding.as_ref(),
        )
    }

    /// binding cannot be a view
    pub fn binding_cannot_be_view(binding: &Binding) -> Self {
        Self::new(
            Kind::BindingCannotBeView,
            "binding cannot be a view",
            binding.as_ref(),
        )
    }

    /// binding cannot be dynamic
    pub fn binding_cannot_be_dynamic(binding: &Binding) -> Self {
        Self::new(
            Kind::BindingCannotBeView,
            "binding cannot be dynamic",
            binding.as_ref(),
        )
    }

    /// unexpected transition
    pub fn unexpected_transition(offending: &impl Spanned) -> Self {
        Self::new(Kind::UnexpectedBinding, "unexpected transition", offending)
    }

    /// expected transition
    pub fn expected_transition(offending: &impl Spanned) -> Self {
        Self::new(Kind::ExpectedTransition, "expected transition", offending)
    }

    /// "foo" is entitled to [E0, E1, ...] in field "bar" which must be provided
    pub fn missing_entitlements(
        offending: &Ident,
        entitlement_peripheral: &Ident,
        entitlement_register: &Ident,
        entitlement_field: &Ident,
        entitlement_variants: impl Iterator<Item = Ident>,
    ) -> Self {
        let entitlement_list = entitlement_variants
            .map(|ident| ident.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Self::new(
            Kind::MissingEntitlements,
            format!(
                "\"{offending}\" is entitled to [{entitlement_list}] in field \
                \"{entitlement_peripheral}::{entitlement_register}::{entitlement_field}\" which must be provided"
            ),
            offending,
        )
    }

    /// missing field "foo" must be provided
    pub fn missing_concrete_field(offending: &Ident, field_ident: &Ident) -> Self {
        Self::new(
            Kind::MissingFields,
            format!("missing field \"{field_ident}\" must be provided"),
            offending,
        )
    }

    /// missing fields [foo, bar, ...] must be provided
    pub fn missing_concrete_fields<'a>(
        offending: &Ident,
        field_idents: impl Iterator<Item = &'a Ident>,
    ) -> Self {
        let formatted_fields = field_idents
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        Self::new(
            Kind::MissingFields,
            format!("missing fields [{formatted_fields}] must be provided"),
            offending,
        )
    }

    /// missing fields are ambiguous, but may include any of [foo, bar, ...] which must be provided
    pub fn missing_ambiguous_fields<'a>(
        offending: &Ident,
        field_idents: impl Iterator<Item = &'a Ident>,
    ) -> Self {
        let formatted_fields = field_idents
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        Self::new(
            Kind::MissingFields,
            format!(
                "missing fields are ambiguous, but may include any of [{formatted_fields}] which must be provided"
            ),
            offending,
        )
    }

    /// "foo" is fundamental and as such cannot be masked nor unmasked
    pub fn cannot_unmask_fundamental<'a>(peripheral_ident: &Ident) -> Self {
        Self::new(
            Kind::CannotUnmaskFundamental,
            format!(
                "peripheral \"{peripheral_ident}\" is fundamental and as such cannot be masked nor unmasked"
            ),
            peripheral_ident,
        )
    }

    /// field "foo" is not entitled to nor has entitlements within this gate
    pub fn unincumbent_field<'a>(field_ident: &Ident) -> Self {
        Self::new(
            Kind::UnincumbentField,
            format!(
                "field \"{field_ident}\" is not entitled to nor has entitlements within this gate"
            ),
            field_ident,
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
        let code = format!("[E{:04}]", diagnostic.kind as u32);
        Self::new(diagnostic.span, format!("{code} {}", diagnostic.message()))
    }
}

impl From<syn::Error> for Diagnostic {
    fn from(err: syn::Error) -> Self {
        Self {
            kind: Kind::Erased,
            message: err.to_string(),
            span: err.span(),
        }
    }
}
