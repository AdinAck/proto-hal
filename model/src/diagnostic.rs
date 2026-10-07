//! The model components related to diagnostics emitted while validating a [model](crate).

use std::{fmt::Display, ops::Range};

use diagnostic::{Class, Code, Diagnostic, Label, conjunction};
use source::{Span, Spanned};

use crate::{
    Node, WideWord, Word,
    entitlement::{Entitlement, Space},
    field::{Domain, Field, access_modality},
    peripheral::Peripheral,
    register::Register,
    schema::{AccessSpace, Schema},
    variant::{AccessQualifier, Inert, Variant},
    word_size::WordSize,
};

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
    /// Variants occupying the same discriminant within an [access space](crate::schema#access-spaces).
    VariantOverlap,
    /// An [inert](crate::variant#inert) read variant.
    InertReadVariant,
    /// A discriminant exceeding a field domain.
    DiscriminantOutOfRange,
    /// A variant that [cannot be exposed](crate::field#compatibility) by a field.
    InexposableVariant,
    /// A field [incompatible](crate::field#compatibility) with the schema the field assumes.
    IncompatibleField,
    /// A field that [supports](crate::field#compatibility) read access but exposes no
    /// [readable](crate::schema#access-specificity) variants.
    NoReadableVariants,
    /// A field that [supports](crate::field#compatibility) write access but exposes no
    /// [writable](crate::schema#access-specificity) variants.
    NoWritableVariants,
    /// A reset value exceeding the domain of a [numeric](crate::field#numericity) field.
    ResetOutOfRange,
    /// A reset defined by a field that is not a [store field](crate::field#resets).
    IgnoredReset,
    /// A reset value identifying no variant exposed by an [enumerated](crate::field#numericity) field.
    UninhabitableReset,
    /// A [store field](crate::field#resets) without a reset.
    MissingReset,
    /// An [entitlement](crate::entitlement) value identifying no variant exposed by an
    /// [enumerated](crate::field#numericity) field.
    UninhabitableEntitlement,
    /// An [entitlement](crate::entitlement) value exceeding the domain of the implicated
    /// [numeric](crate::field#numericity) field.
    EntitlementOutOfRange,
    /// A variant imposing [statewise](crate::entitlement#statewise) entitlements within a field the variant is
    /// [foreign](crate::field#schema) to.
    ForeignVariant,
    /// An [entitlement](crate::entitlement) implicating an [unresolvable](crate::field#resolvability) field.
    UnresolvableEntitlement,
    /// An [entitlement space](crate::entitlement#imposing-entitlements) the modality of the imposing field does not
    /// permit.
    UnpermittedEntitlementSpace,
    /// A word size exceeding the permitted range.
    InvalidWordSize,
    /// An address that is not word aligned.
    AddressUnaligned,
    /// An address outside the permitted address space.
    AddressOutOfRange,
    /// Peripherals occupying the same addresses.
    PeripheralOverlap,
    /// Registers occupying the same bytes.
    RegisterOverlap,
    /// Fields occupying the same bits.
    FieldOverlap,
    /// A field domain exceeding the parent register domain.
    ExceedsDomain,
    /// An access width that does not divide the word size.
    InvalidAccessWidth,
}

impl diagnostic::Kind for Structure {
    const CLASS: Class = Class::Structural;

    fn code(self) -> Code {
        Code::new(self as u16)
    }
}

/// "{node} `{name}` is defined more than once".
pub(crate) fn duplicate(node: Node, name: &Spanned<String>, first: Span) -> Diagnostic {
    Diagnostic::error(
        Structure::Duplicate,
        format!("{node} `{}` is defined more than once", **name),
        Label::new("duplicate definition here", name.span),
    )
    .supporting_label(Label::new("first definition here", first))
}

// TODO: a correct frontend shouldn't produce empty inherent schemas, as a field without variants is numeric, so the
//       inherent case is caused by a frontend bug, and should indicate as such, as rustc does for internal compiler
//       errors
/// "assumed schema `{schema}` has no variants". When the provided schema is inherent to the provided field, "inherent
/// schema of field `{field}` has no variants". When no field is provided for an inherent schema, "inherent schema has
/// no variants".
pub(crate) fn empty_schema(schema: &Schema, field: Option<&Field>) -> Diagnostic {
    let subject = match (&schema.name, field) {
        (Some(name), ..) => format!("assumed schema `{}`", **name),
        (None, Some(field)) => format!("inherent schema of field `{}`", *field.name),
        (None, None) => schema_descriptor(schema),
    };

    Diagnostic::warning(
        Structure::EmptySchema,
        format!("{subject} has no variants"),
        Label::new(
            "declared here",
            schema.name.as_ref().map_or(schema.span, |name| name.span),
        ),
    )
}

// TODO: a correct frontend shouldn't produce unused schemas, so this diagnostic is caused by a frontend bug, and should
//       indicate as such, as rustc does for internal compiler errors
/// "schema `{schema}` is never used", or "inherent schema is never used".
pub(crate) fn unused_schema(schema: &Schema) -> Diagnostic {
    Diagnostic::warning(
        Structure::UnusedSchema,
        format!("{} is never used", schema_descriptor(schema)),
        Label::new(
            "declared here",
            schema.name.as_ref().map_or(schema.span, |name| name.span),
        ),
    )
}

/// "variants {names} overlap, occupying {access space} discriminant `{discriminant}`". When no access space is
/// provided, "variants {names} overlap, occupying discriminant `{discriminant}`".
///
/// Labels each of the provided variants with "`{name}` assigned here".
///
/// note: "a discriminant may identify at most one variant within each access space"
///
/// # Panics
///
/// If no variants are provided.
pub(crate) fn variant_overlap<'a>(
    variants: impl IntoIterator<Item = &'a Variant>,
    space: Option<AccessSpace>,
) -> Diagnostic {
    let variants = variants.into_iter().collect::<Vec<_>>();
    let (first, others) = variants.split_first().expect("an overlap has variants");

    let region = match space {
        Some(space) => format!(
            "{space} discriminant `{}`",
            format_value(*first.discriminant)
        ),
        None => format!("discriminant `{}`", format_value(*first.discriminant)),
    };

    let diagnostic = Diagnostic::error(
        Structure::VariantOverlap,
        format!(
            "variants {} overlap, occupying {region}",
            conjunction(
                variants
                    .iter()
                    .map(|variant| format!("`{}`", *variant.name))
            )
        ),
        Label::new(
            format!("`{}` assigned here", *first.name),
            first.discriminant.span,
        ),
    )
    .note("a discriminant may identify at most one variant within each access space");

    others.iter().fold(diagnostic, |diagnostic, variant| {
        diagnostic.supporting_label(Label::new(
            format!("`{}` assigned here", *variant.name),
            variant.discriminant.span,
        ))
    })
}

/// "read variant `{name}` cannot be inert".
///
/// note: "inertness is a property of writable variants"
///
/// # Panics
///
/// If the provided variant is not inert.
pub(crate) fn inert_read_variant(variant: &Variant) -> Diagnostic {
    Diagnostic::error(
        Structure::InertReadVariant,
        format!("read variant `{}` cannot be inert", *variant.name),
        Label::new(
            "inert qualifier",
            variant.inert.expect("an inert read variant is inert").span,
        ),
    )
    .note("inertness is a property of writable variants")
}

/// "variant `{variant}` with discriminant `{discriminant}` exceeds field `{field}` of width `{width}`".
///
/// note: "discriminant `{discriminant}` does not satisfy `discriminant <= {max value}`"
pub(crate) fn discriminant_out_of_range(variant: &Variant, field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::DiscriminantOutOfRange,
        format!(
            "variant `{}` with discriminant `{}` exceeds field `{}` of width `{}`",
            *variant.name,
            format_value(*variant.discriminant),
            *field.name,
            field.domain.width(),
        ),
        Label::new("out-of-range discriminant", variant.discriminant.span),
    )
    .supporting_label(Label::new(
        format!("domain of field `{}`", *field.name),
        field.domain.span,
    ))
    .note(format!(
        "discriminant `{}` does not satisfy `discriminant <= {}`",
        format_value(*variant.discriminant),
        format_value(field.domain.max_value())
    ))
}

/// The qualifier preventing a field from [exposing](crate::field#compatibility) a variant.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Inexposable<'a> {
    /// An access qualifier.
    Access(&'a Spanned<AccessQualifier>),
    /// The inert qualifier.
    Inert(&'a Spanned<Inert>),
}

// TODO: help: "consider modality `read write` for field `{field}`"
// TODO: for inherent schemas, help: "consider removing qualifier `{qualifier}` from variant `{variant}`"
/// "{qualifier} variant `{variant}` cannot be exposed by field `{field}` with modality `{modality}`", such as "write
/// variant `foo` cannot be exposed by field `bar` with modality `read`".
///
/// When the modality of the provided field is `read`, note: "variants that imply write access cannot be exposed by
/// fields with modality `read`". When the modality of the provided field is `write`, note: "variants that imply read
/// access cannot be exposed by fields with modality `write`". When the modality of the provided field is `store` or
/// `volatile store`, note: "access-specific variants cannot be exposed by fields with modality `{modality}`" for an
/// access qualifier, and "inert variants cannot be exposed by fields with modality `{modality}`" for the inert
/// qualifier.
///
/// # Panics
///
/// If the modality of the provided field is `read write`.
pub(crate) fn inexposable_variant(
    variant: &Variant,
    field: &Field,
    qualifier: Inexposable,
) -> Diagnostic {
    use access_modality::*;

    let modality = *field.access_modality;

    let note = match (modality, qualifier) {
        (Read, ..) => {
            "variants that imply write access cannot be exposed by fields with modality `read`"
                .to_string()
        }
        (Write, ..) => {
            "variants that imply read access cannot be exposed by fields with modality `write`"
                .to_string()
        }
        (Store | VolatileStore, Inexposable::Access(..)) => {
            format!(
                "access-specific variants cannot be exposed by fields with modality `{modality}`"
            )
        }
        (Store | VolatileStore, Inexposable::Inert(..)) => {
            format!("inert variants cannot be exposed by fields with modality `{modality}`")
        }
        (ReadWrite, ..) => {
            unreachable!("fields with modality `read write` can expose all variants")
        }
    };

    let (qualifier, span) = match qualifier {
        Inexposable::Access(access) => (access.to_string(), access.span),
        Inexposable::Inert(inert) => ("inert".to_string(), inert.span),
    };

    Diagnostic::error(
        Structure::InexposableVariant,
        format!(
            "{qualifier} variant `{}` cannot be exposed by field `{}` with modality `{modality}`",
            *variant.name, *field.name
        ),
        Label::new(format!("{qualifier} qualifier"), span),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ))
    .note(note)
}

/// "field `{field}` with modality `{modality}` cannot assume schema `{schema}`", containing the provided diagnostics as
/// [children](::diagnostic#children).
///
/// # Panics
///
/// If the provided field assumes no schema.
pub(crate) fn incompatible_field(
    field: &Field,
    schema: &Schema,
    children: impl IntoIterator<Item = Diagnostic>,
) -> Diagnostic {
    let diagnostic = Diagnostic::error(
        Structure::IncompatibleField,
        format!(
            "field `{}` with modality `{}` cannot assume {}",
            *field.name,
            *field.access_modality,
            schema_descriptor(schema)
        ),
        Label::new(
            "incompatible schema",
            field
                .schema
                .as_ref()
                .expect("an incompatible field exposes a schema")
                .span,
        ),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ));

    children.into_iter().fold(diagnostic, Diagnostic::child)
}

/// "field `{field}` supports read access but exposes no readable variants".
///
/// When the provided field assumes the schema, note: "assumed schema `{schema}` defines no readable variants".
///
/// # Panics
///
/// If the provided field has no schema.
pub(crate) fn no_readable_variants(field: &Field, schema: &Schema) -> Diagnostic {
    missing_variants(
        Structure::NoReadableVariants,
        "read",
        "readable",
        field,
        schema,
    )
}

/// "field `{field}` supports write access but exposes no writable variants".
///
/// When the provided field assumes the schema, note: "assumed schema `{schema}` defines no writable variants".
///
/// # Panics
///
/// If the provided field has no schema.
pub(crate) fn no_writable_variants(field: &Field, schema: &Schema) -> Diagnostic {
    missing_variants(
        Structure::NoWritableVariants,
        "write",
        "writable",
        field,
        schema,
    )
}

/// Create a warning of the provided kind, stating that the provided field supports the access named by the provided
/// word, such as "read", but exposes no variants described by the provided adjective, such as "readable".
fn missing_variants(
    kind: Structure,
    access: &str,
    adjective: &str,
    field: &Field,
    schema: &Schema,
) -> Diagnostic {
    let diagnostic = Diagnostic::warning(
        kind,
        format!(
            "field `{}` supports {access} access but exposes no {adjective} variants",
            *field.name
        ),
        Label::new(
            format!("schema without {adjective} variants"),
            field
                .schema
                .as_ref()
                .expect("a field missing variants has a schema")
                .span,
        ),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ));

    // note: for an inherent schema, the note would only restate the message
    match &schema.name {
        Some(name) => diagnostic.note(format!(
            "assumed schema `{}` defines no {adjective} variants",
            **name
        )),
        None => diagnostic,
    }
}

/// The descriptor of the provided schema for use in diagnostic messages, such as "schema `foo`", or "inherent schema"
/// when the schema has no name.
fn schema_descriptor(schema: &Schema) -> String {
    match &schema.name {
        Some(name) => format!("schema `{}`", **name),
        None => "inherent schema".to_string(),
    }
}

/// "reset `{value}` exceeds field `{field}` of width `{width}`".
///
/// note: "reset `{value}` does not satisfy `reset <= {max value}`"
pub(crate) fn reset_out_of_range(value: Spanned<Word>, field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::ResetOutOfRange,
        format!(
            "reset `{}` exceeds field `{}` of width `{}`",
            format_value(*value),
            *field.name,
            field.domain.width(),
        ),
        Label::new("out-of-range reset", value.span),
    )
    .supporting_label(Label::new(
        format!("domain of field `{}`", *field.name),
        field.domain.span,
    ))
    .note(format!(
        "reset `{}` does not satisfy `reset <= {}`",
        format_value(*value),
        format_value(field.domain.max_value())
    ))
}

/// "reset of field `{field}` with modality `{modality}` is ignored".
///
/// note: "resets are a property of fields with modality `store` or `volatile store`"
///
/// # Panics
///
/// If the provided field has no reset.
pub(crate) fn ignored_reset(field: &Field) -> Diagnostic {
    Diagnostic::warning(
        Structure::IgnoredReset,
        format!(
            "reset of field `{}` with modality `{}` is ignored",
            *field.name, *field.access_modality
        ),
        Label::new(
            "ignored reset",
            field.reset.expect("an ignored reset is defined").span,
        ),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ))
    .note("resets are a property of fields with modality `store` or `volatile store`")
}

/// "reset `{value}` has no corresponding variant exposed by field `{field}`".
///
/// note: "enumerated field reset values must identify exactly one exposed variant"
///
/// # Panics
///
/// If the provided field has no schema.
pub(crate) fn uninhabitable_reset(value: Spanned<Word>, field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::UninhabitableReset,
        format!(
            "reset `{}` has no corresponding variant exposed by field `{}`",
            format_value(*value),
            *field.name
        ),
        Label::new("unidentified reset", value.span),
    )
    .supporting_label(Label::new(
        format!("variants exposed by field `{}`", *field.name),
        field
            .schema
            .expect("an uninhabitable reset is defined by an enumerated field")
            .span,
    ))
    .note("enumerated field reset values must identify exactly one exposed variant")
}

/// "field `{field}` with modality `{modality}` has no reset".
///
/// note: "resets define the statically known initial state of fields with modality `{modality}`, which is required"
pub(crate) fn missing_reset(field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::MissingReset,
        format!(
            "field `{}` with modality `{}` has no reset",
            *field.name, *field.access_modality
        ),
        Label::new("field without reset", field.name.span),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ))
    .note(format!(
        "resets define the statically known initial state of fields with modality `{}`, which is required",
        *field.access_modality
    ))
}

/// "entitlement value `{value}` has no corresponding variant exposed by field `{field}`".
///
/// note: "enumerated field entitlement values must identify exactly one exposed variant"
///
/// # Panics
///
/// If the provided field has no schema.
pub(crate) fn uninhabitable_entitlement(value: Spanned<Word>, field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::UninhabitableEntitlement,
        format!(
            "entitlement value `{}` has no corresponding variant exposed by field `{}`",
            format_value(*value),
            *field.name
        ),
        Label::new("unidentified entitlement value", value.span),
    )
    .supporting_label(Label::new(
        format!("variants exposed by field `{}`", *field.name),
        field
            .schema
            .expect("an uninhabitable entitlement value implicates an enumerated field")
            .span,
    ))
    .note("enumerated field entitlement values must identify exactly one exposed variant")
}

/// "entitlement value `{value}` exceeds field `{field}` of width `{width}`".
///
/// note: "entitlement value `{value}` does not satisfy `value <= {max value}`"
pub(crate) fn entitlement_out_of_range(value: Spanned<Word>, field: &Field) -> Diagnostic {
    Diagnostic::error(
        Structure::EntitlementOutOfRange,
        format!(
            "entitlement value `{}` exceeds field `{}` of width `{}`",
            format_value(*value),
            *field.name,
            field.domain.width(),
        ),
        Label::new("out-of-range entitlement value", value.span),
    )
    .supporting_label(Label::new(
        format!("domain of field `{}`", *field.name),
        field.domain.span,
    ))
    .note(format!(
        "entitlement value `{}` does not satisfy `value <= {}`",
        format_value(*value),
        format_value(field.domain.max_value())
    ))
}

/// "entitlement targets unresolvable field `{field}` with modality `{modality}`".
///
/// note: "only resolvable fields inhabit states"
///
/// When the modality of the provided field is `volatile store`, note: "fields with modality `volatile store` are
/// resolvable only when hardware affordance entitlements are non-tautological".
pub(crate) fn unresolvable_entitlement(
    entitlement: &Spanned<Entitlement>,
    field: &Field,
) -> Diagnostic {
    let modality = *field.access_modality;

    let diagnostic = Diagnostic::error(
        Structure::UnresolvableEntitlement,
        format!(
            "entitlement targets unresolvable field `{}` with modality `{modality}`",
            *field.name
        ),
        Label::new("entitlement to unresolvable field", entitlement.span),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ))
    .note("only resolvable fields inhabit states");

    match modality {
        access_modality::VolatileStore => diagnostic.note(
            "fields with modality `volatile store` are resolvable only when hardware affordance entitlements \
             are non-tautological",
        ),
        _ => diagnostic,
    }
}

/// "field `{field}` with modality `read` cannot impose affordance entitlements".
///
/// note: "affordance entitlements constrain software writes, which fields with modality `read` do not support"
///
/// # Panics
///
/// If the provided field imposes no affordance entitlements.
pub(crate) fn unpermitted_affordance(field: &Field) -> Diagnostic {
    unpermitted_entitlement_space(
        "affordance",
        field
            .affordance_entitlements
            .as_ref()
            .expect("unpermitted affordance entitlements are imposed"),
        field,
    )
    .note(format!(
        "affordance entitlements constrain software writes, which fields with modality `{}` do not support",
        *field.access_modality
    ))
}

/// "field `{field}` with modality `{modality}` cannot impose hardware affordance entitlements".
///
/// note: "hardware affordance entitlements only serve to determine whether fields with modality `volatile store` are
/// resolvable"
///
/// # Panics
///
/// If the provided field imposes no hardware affordance entitlements.
pub(crate) fn unpermitted_hardware_affordance(field: &Field) -> Diagnostic {
    unpermitted_entitlement_space(
        "hardware affordance",
        field
            .hardware_affordance_entitlements
            .as_ref()
            .expect("unpermitted hardware affordance entitlements are imposed"),
        field,
    )
    .note(
        "hardware affordance entitlements only serve to determine whether fields with modality `volatile store` are \
         resolvable",
    )
}

/// "field `{field}` with modality `{modality}` cannot impose {quality} entitlements", where the quality names the
/// provided space, such as "affordance".
fn unpermitted_entitlement_space(
    quality: &str,
    space: &Spanned<Space>,
    field: &Field,
) -> Diagnostic {
    Diagnostic::error(
        Structure::UnpermittedEntitlementSpace,
        format!(
            "field `{}` with modality `{}` cannot impose {quality} entitlements",
            *field.name, *field.access_modality
        ),
        Label::new(
            format!("unpermitted {quality} entitlement space"),
            space.span,
        ),
    )
    .supporting_label(Label::new(
        format!("modality of field `{}`", *field.name),
        field.access_modality.span,
    ))
}

// TODO: a correct frontend shouldn't produce statewise entitlements of foreign variants, so this diagnostic is caused
//       by a frontend bug, and should indicate as such, as rustc does for internal compiler errors
/// "statewise entitlement variant `{variant}` is foreign to field `{field}`".
///
/// note: "fields can only inhabit the variants they expose"
pub(crate) fn foreign_statewise_variant(
    variant: &Variant,
    space: &Spanned<Space>,
    field: &Field,
) -> Diagnostic {
    Diagnostic::error(
        Structure::ForeignVariant,
        format!(
            "statewise entitlement variant `{}` is foreign to field `{}`",
            *variant.name, *field.name
        ),
        Label::new("statewise entitlement space of foreign variant", space.span),
    )
    .supporting_label(Label::new(
        format!("definition of variant `{}`", *variant.name),
        variant.name.span,
    ))
    .note("fields can only inhabit the variants they expose")
}

/// "word size must be from {min} to {max} bytes".
///
/// note: "word size `{word size}` does not satisfy `{min} <= word size <= {max}`"
pub(crate) fn invalid_word_size(word_size: &Spanned<Word>) -> Diagnostic {
    let (min, max) = WordSize::SIZES.into_inner();

    Diagnostic::error(
        Structure::InvalidWordSize,
        format!("word size must be from {min} to {max} bytes"),
        Label::new("invalid word size", word_size.span),
    )
    .note(format!(
        "word size `{}` does not satisfy `{min} <= word size <= {max}`",
        **word_size
    ))
}

/// "{node} address must be word aligned".
///
/// note: "address `{address}` does not satisfy `address % {word size} == 0`"
pub(crate) fn address_unaligned(
    node: Node,
    address: &Spanned<Word>,
    word_size: WordSize,
) -> Diagnostic {
    Diagnostic::error(
        Structure::AddressUnaligned,
        format!("{node} address must be word aligned"),
        Label::new("unaligned address", address.span),
    )
    .note(format!(
        "address `{}` does not satisfy `address % {word_size} == 0`",
        format_address(WideWord::from(**address), word_size)
    ))
}

/// "{node} address must reside within the address space".
///
/// note: "address `{address}` does not satisfy `address <= {last address}`"
pub(crate) fn address_out_of_range(
    node: Node,
    address: Spanned<WideWord>,
    word_size: WordSize,
) -> Diagnostic {
    Diagnostic::error(
        Structure::AddressOutOfRange,
        format!("{node} address must reside within the address space"),
        Label::new("out-of-range address", address.span),
    )
    .note(format!(
        "address `{}` does not satisfy `address <= {}`",
        format_address(*address, word_size),
        format_address(WideWord::from(word_size.last_address()), word_size)
    ))
}

/// "peripherals {names} overlap, occupying bytes `{region}`".
///
/// Labels each of the affected registers (those occupying the overlap region) with "register
/// `{peripheral}::{register}` overlaps".
///
/// # Panics
///
/// If no peripherals are provided.
pub(crate) fn peripheral_overlap<'a>(
    peripherals: impl IntoIterator<Item = (&'a Peripheral, impl IntoIterator<Item = &'a Register>)>,
    region: impl Display,
) -> Diagnostic {
    let peripherals = peripherals
        .into_iter()
        .map(|(peripheral, registers)| (peripheral, registers.into_iter().collect::<Vec<_>>()))
        .collect::<Vec<_>>();

    let diagnostic = overlap(
        Structure::PeripheralOverlap,
        Node::Peripheral,
        peripherals
            .iter()
            .map(|(peripheral, ..)| (&peripheral.name, peripheral.address.span)),
        "bytes",
        region,
    );

    peripherals
        .iter()
        .flat_map(|(peripheral, registers)| {
            registers.iter().map(move |register| (peripheral, register))
        })
        .fold(diagnostic, |diagnostic, (peripheral, register)| {
            diagnostic.supporting_label(Label::new(
                format!(
                    "register `{}::{}` overlaps",
                    *peripheral.name, *register.name
                ),
                register.offset.span,
            ))
        })
}

/// "registers {names} overlap, occupying bytes `{region}`".
///
/// # Panics
///
/// If no registers are provided.
pub(crate) fn register_overlap<'a>(
    registers: impl IntoIterator<Item = &'a Register>,
    region: impl Display,
) -> Diagnostic {
    overlap(
        Structure::RegisterOverlap,
        Node::Register,
        registers
            .into_iter()
            .map(|register| (&register.name, register.offset.span)),
        "bytes",
        region,
    )
}

/// "fields {names} overlap, occupying bits `{region}`".
///
/// # Panics
///
/// If no fields are provided.
pub(crate) fn field_overlap<'a>(
    fields: impl IntoIterator<Item = &'a Field>,
    region: impl Display,
) -> Diagnostic {
    overlap(
        Structure::FieldOverlap,
        Node::Field,
        fields
            .into_iter()
            .map(|field| (&field.name, field.domain.span)),
        "bits",
        region,
    )
}

/// "field `{name}` with domain `{domain}` exceeds parent register with domain `{register}`".
pub(crate) fn exceeds_domain(field: &Field, register: Domain) -> Diagnostic {
    Diagnostic::error(
        Structure::ExceedsDomain,
        format!(
            "field `{}` with domain `{}` exceeds parent register with domain `{register}`",
            *field.name, *field.domain
        ),
        Label::new("exceeding field domain", field.domain.span),
    )
}

/// "access width `{width}` does not divide the word size `{word size}`".
pub(crate) fn invalid_access_width(width: &Spanned<Word>, word_size: WordSize) -> Diagnostic {
    Diagnostic::error(
        Structure::InvalidAccessWidth,
        format!(
            "access width `{}` does not divide the word size `{word_size}`",
            **width
        ),
        Label::new("invalid access width", width.span),
    )
}

/// Create a diagnostic of the provided kind, labelling where each of the provided occupants was placed. The occupants
/// are siblings of the provided node kind, overlapping in the provided region, which is measured in the provided unit,
/// such as "bits".
fn overlap<'a>(
    kind: Structure,
    node: Node,
    occupants: impl IntoIterator<Item = (&'a Spanned<String>, Span)>,
    unit: &str,
    region: impl Display,
) -> Diagnostic {
    let occupants = occupants.into_iter().collect::<Vec<_>>();
    let ((first, placement), others) = occupants.split_first().expect("an overlap has occupants");

    let diagnostic = Diagnostic::error(
        kind,
        format!(
            "{node}s {} overlap, occupying {unit} `{region}`",
            conjunction(occupants.iter().map(|(name, ..)| format!("`{}`", **name)))
        ),
        Label::new(format!("`{}` placed here", **first), *placement),
    );

    others
        .iter()
        .fold(diagnostic, |diagnostic, (name, placement)| {
            diagnostic.supporting_label(Label::new(format!("`{}` placed here", **name), *placement))
        })
}

/// Display the provided address, padded to the word size and grouped into four digits, for use in diagnostic messages,
/// such as "0x0000_0004". An address outside the [address space](crate#address-space) is padded to whole groups, such
/// as "0x0001_0000_0000".
fn format_address(address: WideWord, word_size: WordSize) -> String {
    let digits = format!("{address:x}");
    let word_width = usize::from(word_size.bytes()) * 2; // each character is half a byte

    let width = if digits.len() > word_width {
        digits.len().next_multiple_of(4)
    } else {
        word_width
    };

    format_hex(&format!("{digits:0>width$}"))
}

/// Display the provided value in hex, grouped into four digits, for use in diagnostic messages, such as "0x1_0000".
fn format_value(value: Word) -> String {
    format_hex(&format!("{value:x}"))
}

/// Prefix the provided hex digits with "0x", grouping them into four digits from the right, such as "0x1_0000".
fn format_hex(digits: &str) -> String {
    let groups = digits
        .as_bytes()
        .rchunks(4)
        .rev()
        .map(|group| str::from_utf8(group).expect("hex digits are ascii"))
        .collect::<Vec<_>>();

    format!("0x{}", groups.join("_"))
}

/// Display the provided range of bytes, padded to the word size and grouped into four digits, for use in diagnostic
/// messages, such as "0x0000_0004..0x0000_0008".
pub(crate) fn format_bytes(range: &Range<WideWord>, word_size: WordSize) -> String {
    format!(
        "{}..{}",
        format_address(range.start, word_size),
        format_address(range.end, word_size)
    )
}

/// Display the provided range of bits for use in diagnostic messages, such as "3:2".
pub(crate) fn format_bits(range: &Range<WideWord>) -> String {
    format!("{}:{}", range.end - 1, range.start)
}
