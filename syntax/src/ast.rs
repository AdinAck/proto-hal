//! The abstract syntax tree of phm.
//!
//! The types here are *syntactic*: template invocations, arrays, schema references,
//! and `requires` clauses are preserved exactly as written. Elaboration into a
//! device model — template expansion, name resolution, requiredness — is a separate
//! pass (in `proto-hal-build`) which queries this tree and emits semantic
//! diagnostics using the retained [`Span`]s.
//!
//! # Structural principles
//!
//! - **Braces always denote children.** Everything *about* an item — position, value,
//!   reset, schema references, `requires` clauses — appears in its head, before the body.
//! - **Symbols are contextual operators**, not general syntax: `~` exists only after a
//!   variant's name, `reset` only on registers and fields, and so on. Consequently the
//!   kinds are distinct types, and a definition that parses cannot ascribe a property
//!   to a kind that does not support it.
//! - **Placement is typed.** Each container holds an item enum of exactly the kinds
//!   that may appear within it.
//!
//! What remains for elaboration: requiredness after template merging ("neither the
//! template nor the invocation specify an offset"), reference resolution, and
//! cross-item judgements (overlap, agreement, coverage).

mod common;
mod device;
mod entitlement;
mod field;
mod group;
mod interrupts;
mod peripheral;
mod register;
mod schema;
mod variant;

pub use common::{
    Access, Domain, Head, Indices, ListEntry, NumRange, Path, ResetValue, Rest, Side, Stride,
};
pub use device::{Device, DeviceItem};
pub use entitlement::{Entitled, Pattern, Segment, Space};
pub use field::{Field, FieldItem, FieldRequires};
pub use group::{FieldGroup, Group, GroupItem, PeripheralGroup, RegisterGroup};
pub use interrupts::{InterruptEntry, InterruptKind, Interrupts};
pub use peripheral::{Peripheral, PeripheralItem};
pub use register::{Register, RegisterItem};
pub use schema::{Schema, SchemaItem};
pub use variant::{ValueEntry, Variant, VariantValue};

use chumsky::span::SimpleSpan;

/// Identifies the source file a [`Span`] points into: an index into the
/// loaded sources, assigned by the caller of [`parse`](crate::parse).
pub type SourceId = usize;

/// A byte range within a source file, carrying the [`SourceId`] of the file
/// it points into — so every diagnostic self-identifies its origin, even
/// within trees merged across files.
pub type Span = SimpleSpan<usize, SourceId>;

/// A syntax node paired with the [`Span`] it was parsed from.
pub type Spanned<T> = chumsky::span::Spanned<T, Span>;

/// A single source file: a sequence of items.
///
/// Top-level items are *noncorporeal* definitions. Items nested within another
/// item's body are *elaborated* into it.
#[derive(Debug, Clone, PartialEq)]
pub struct File<'src> {
    pub items: Vec<Spanned<FileItem<'src>>>,
}

/// Any definition may exist at the top level of a file, where it is usable
/// as a template.
#[derive(Debug, Clone, PartialEq)]
pub enum FileItem<'src> {
    Import(Import<'src>),
    Device(Device<'src>),
    Peripheral(Peripheral<'src>),
    PeripheralGroup(PeripheralGroup<'src>),
    Register(Register<'src>),
    RegisterGroup(RegisterGroup<'src>),
    Field(Field<'src>),
    FieldGroup(FieldGroup<'src>),
    Schema(Schema<'src>),
    Variant(Variant<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}

/// `import path` — bring another file's definitions into scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Import<'src> {
    pub path: Spanned<Path<'src>>,
}
