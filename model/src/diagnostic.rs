//! The model components related to diagnostics emitted while validating a [model](crate).

use std::{fmt::Display, ops::Range};

use diagnostic::{Class, Code, Diagnostic, Label, conjunction};
use source::{Span, Spanned};

use crate::{
    Node, WideWord, Word, WordSize,
    field::{Domain, Field},
    peripheral::Peripheral,
    register::Register,
    schema::Schema,
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
pub fn duplicate(node: Node, name: &Spanned<String>, first: Span) -> Diagnostic {
    Diagnostic::error(
        Structure::Duplicate,
        format!("{node} `{}` is defined more than once", **name),
        Label::new("duplicate definition here", name.span),
    )
    .supporting_label(Label::new("first definition here", first))
}

/// "schema `{name}` has no variants", or "schema has no variants".
pub fn empty_schema(schema: &Schema) -> Diagnostic {
    Diagnostic::warning(
        Structure::EmptySchema,
        format!("{} has no variants", self::schema(schema)),
        Label::new(
            "declared here",
            schema.name.as_ref().map_or(schema.span, |name| name.span),
        ),
    )
}

// TODO: a correct frontend shouldn't produce unused schemas, so this diagnostic is caused by a frontend bug, and should
//       indicate as such, as rustc does for internal compiler errors
/// "schema `{name}` is never used", or "schema is never used".
pub fn unused_schema(schema: &Schema) -> Diagnostic {
    Diagnostic::warning(
        Structure::UnusedSchema,
        format!("{} is never used", self::schema(schema)),
        Label::new(
            "declared here",
            schema.name.as_ref().map_or(schema.span, |name| name.span),
        ),
    )
}

/// Produce a description of the provided schema for use in diagnostic messages, such as "schema `foo`", or "schema"
/// when it has no name.
fn schema(schema: &Schema) -> String {
    match &schema.name {
        Some(name) => format!("schema `{}`", **name),
        None => "schema".to_string(),
    }
}

/// "word size must be from {min} to {max} bytes".
///
/// note: "word size `{word size}` does not satisfy `{min} <= word size <= {max}`"
pub fn invalid_word_size(word_size: &Spanned<Word>) -> Diagnostic {
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
pub fn address_unaligned(node: Node, address: &Spanned<Word>, word_size: WordSize) -> Diagnostic {
    Diagnostic::error(
        Structure::AddressUnaligned,
        format!("{node} address must be word aligned"),
        Label::new("unaligned address", address.span),
    )
    .note(format!(
        "address `{}` does not satisfy `address % {word_size} == 0`",
        self::address(WideWord::from(**address), word_size)
    ))
}

/// "{node} address must reside within the address space".
///
/// note: "address `{address}` does not satisfy `address <= {last address}`"
pub fn address_out_of_range(
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
        self::address(*address, word_size),
        self::address(WideWord::from(word_size.last_address()), word_size)
    ))
}

/// "peripherals {names} overlap, occupying `{region}`".
///
/// Labels each of the affected registers (those occupying the overlap region) with "register
/// `{peripheral}::{register}` overlaps".
///
/// # Panics
///
/// If no peripherals are provided.
pub fn peripheral_overlap<'a>(
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

/// "registers {names} overlap, occupying `{region}`".
///
/// # Panics
///
/// If no registers are provided.
pub fn register_overlap<'a>(
    registers: impl IntoIterator<Item = &'a Register>,
    region: impl Display,
) -> Diagnostic {
    overlap(
        Structure::RegisterOverlap,
        Node::Register,
        registers
            .into_iter()
            .map(|register| (&register.name, register.offset.span)),
        region,
    )
}

/// "fields {names} overlap, occupying `{region}`".
///
/// # Panics
///
/// If no fields are provided.
pub fn field_overlap<'a>(
    fields: impl IntoIterator<Item = &'a Field>,
    region: impl Display,
) -> Diagnostic {
    overlap(
        Structure::FieldOverlap,
        Node::Field,
        fields
            .into_iter()
            .map(|field| (&field.name, field.domain.span)),
        region,
    )
}

/// "field `{name}` with domain `{domain}` exceeds parent register with domain `{register}`".
pub fn exceeds_domain(field: &Field, register: Domain) -> Diagnostic {
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
pub fn invalid_access_width(width: &Spanned<Word>, word_size: WordSize) -> Diagnostic {
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
/// are siblings of the provided node kind, overlapping in the provided region.
fn overlap<'a>(
    kind: Structure,
    node: Node,
    occupants: impl IntoIterator<Item = (&'a Spanned<String>, Span)>,
    region: impl Display,
) -> Diagnostic {
    let occupants = occupants.into_iter().collect::<Vec<_>>();
    let ((first, placement), others) = occupants.split_first().expect("an overlap has occupants");

    let diagnostic = Diagnostic::error(
        kind,
        format!(
            "{node}s {} overlap, occupying `{region}`",
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
fn address(address: WideWord, word_size: WordSize) -> String {
    let digits = format!("{address:x}");
    let word_width = usize::from(word_size.bytes()) * 2; // each character is half a byte

    let width = if digits.len() > word_width {
        digits.len().next_multiple_of(4)
    } else {
        word_width
    };

    let digits = format!("{digits:0>width$}");

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
        address(range.start, word_size),
        address(range.end, word_size)
    )
}

/// Display the provided range of bits for use in diagnostic messages, such as "3:2".
pub(crate) fn format_bits(range: &Range<WideWord>) -> String {
    format!("{}:{}", range.end - 1, range.start)
}
