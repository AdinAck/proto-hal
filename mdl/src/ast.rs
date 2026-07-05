//! The abstract syntax tree of the model description language.
//!
//! The types here are *syntactic*: template invocations, arrays, schema references,
//! and `requires` clauses are preserved exactly as written. Elaboration into a `phm`
//! model — template expansion, name resolution, requiredness — is a separate pass
//! which queries this tree and emits semantic diagnostics using the retained [`Span`]s.
//!
//! Structural principles:
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

use chumsky::span::SimpleSpan;

pub type Span = SimpleSpan;
pub use chumsky::span::Spanned;

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
    /// elaboration should skip these.
    Error,
}

/// The common part of every definition's head: the template it derives from,
/// its name, and its array designators.
///
/// `#template`, `#template as name`, or `name` — where array definitions may
/// suffix the name with [`Indices`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Head<'src> {
    pub template: Option<Spanned<Path<'src>>>,
    pub name: Option<Spanned<&'src str>>,
    pub indices: Option<Spanned<Indices<'src>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub head: Head<'src>,
    pub body: Option<Spanned<Vec<Spanned<DeviceItem<'src>>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceItem<'src> {
    Peripheral(Peripheral<'src>),
    PeripheralGroup(PeripheralGroup<'src>),
    Schema(Schema<'src>),
    Interrupts(Interrupts<'src>),
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peripheral<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub leaky: Option<Span>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the base address(es).
    pub domain: Option<Spanned<Domain>>,
    /// Ontological entitlements: the peripheral exists only when satisfied.
    pub requires: Option<Spanned<Space<'src>>>,
    pub body: Option<Spanned<Vec<Spanned<PeripheralItem<'src>>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeripheralItem<'src> {
    Register(Register<'src>),
    RegisterGroup(RegisterGroup<'src>),
    Schema(Schema<'src>),
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Register<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub leaky: Option<Span>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the offset(s) within the parent peripheral.
    pub domain: Option<Spanned<Domain>>,
    pub reset: Option<Spanned<ResetValue<'src>>>,
    pub body: Option<Spanned<Vec<Spanned<RegisterItem<'src>>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegisterItem<'src> {
    Field(Field<'src>),
    FieldGroup(FieldGroup<'src>),
    Schema(Schema<'src>),
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub leaky: Option<Span>,
    /// Access modality words, as written. Modalities compose as a *set*:
    /// `read write` (in either order) expresses read-write access.
    pub access: Vec<Spanned<Access>>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the bit domain(s) within the parent register.
    pub domain: Option<Spanned<Domain>>,
    /// `extends path` or `assumes path`.
    pub schema: Option<Spanned<SchemaRef<'src>>>,
    pub reset: Option<Spanned<ResetValue<'src>>>,
    pub requires: FieldRequires<'src>,
    pub body: Option<Spanned<Vec<Spanned<FieldItem<'src>>>>>,
}

/// A field's entitlements, one space per kind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FieldRequires<'src> {
    /// `requires ...` — ontological: the field exists only when satisfied.
    pub plain: Option<Spanned<Space<'src>>>,
    /// `write requires ...` — the field is writable only when satisfied.
    pub write: Option<Spanned<Space<'src>>>,
    /// `hardware write requires ...` — hardware writes to the field only
    /// when satisfied.
    pub hardware_write: Option<Spanned<Space<'src>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldItem<'src> {
    Variant(Variant<'src>),
    Error,
}

/// Schema bodies hold variants, exactly as field bodies do.
pub type SchemaItem<'src> = FieldItem<'src>;

#[derive(Debug, Clone, PartialEq)]
pub struct Schema<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub leaky: Option<Span>,
    /// See [`Field::access`].
    pub access: Vec<Spanned<Access>>,
    pub head: Head<'src>,
    pub body: Option<Spanned<Vec<Spanned<SchemaItem<'src>>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub leaky: Option<Span>,
    pub inert: Option<Span>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `~` — the value(s) the variant occupies.
    pub value: Option<Spanned<VariantValue>>,
    /// Statewise entitlements.
    pub requires: Option<Spanned<Space<'src>>>,
}

/// The value(s) a variant occupies.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantValue {
    /// `~ 0x3`
    Value(u32),
    /// `~ [0x0, 0x4, ...]` — one entry per array element.
    List(Vec<Spanned<ValueEntry>>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueEntry {
    Value(u32),
    Rest(Rest),
}

/// A named collection of members which is *not* itself a container: the group's
/// members remain children of the group's parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'src, T> {
    pub docs: Vec<Spanned<&'src str>>,
    pub name: Option<Spanned<&'src str>>,
    pub body: Spanned<Vec<Spanned<GroupItem<'src, T>>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupItem<'src, T> {
    Member(T),
    Schema(Schema<'src>),
    Error,
}

pub type PeripheralGroup<'src> = Group<'src, Peripheral<'src>>;
pub type RegisterGroup<'src> = Group<'src, Register<'src>>;
pub type FieldGroup<'src> = Group<'src, Field<'src>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Store,
    VolatileStore,
}

/// A `.`-separated reference to a definition, schema, or variant.
#[derive(Debug, Clone, PartialEq)]
pub struct Path<'src> {
    pub segments: Vec<Spanned<&'src str>>,
}

/// Array element designators.
#[derive(Debug, Clone, PartialEq)]
pub enum Indices<'src> {
    /// `[a, b, c]` — each element is suffixed with a name.
    Names(Vec<Spanned<&'src str>>),
    /// `[0..=15]` — each element is suffixed with an index.
    Range(NumRange),
}

/// A schema reference.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaRef<'src> {
    /// The field's schema is *inherent*, extending the referenced schema
    /// with its own variants.
    Extends(Path<'src>),
    /// The field's schema is *linked*: it reflects the referenced schema exactly.
    Assumes(Path<'src>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumRange {
    pub start: Spanned<u32>,
    pub end: Spanned<u32>,
    /// `..=` rather than `..`.
    pub inclusive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Domain {
    /// `@ 0xc4` — an address, offset, or single-bit position.
    Value(u32),
    /// `@ 0..=1` — a bit domain.
    Range(NumRange),
    /// `@ [0x0, 0x4]` or `@ [0..=1, ...]` — one entry per array element.
    List(Vec<Spanned<ListEntry>>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListEntry {
    Value(u32),
    Range(NumRange),
    Rest(Rest),
}

/// `...` — the remaining elements continue the pattern.
///
/// Without a [stride](Rest::stride), the step is positional: field domains pack
/// adjacently by width, register offsets step by the register size, and variant
/// values increment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rest {
    /// `+0x4` / `-0x4` — overrides the default step.
    pub stride: Option<Spanned<Stride>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stride {
    /// `-` rather than `+`.
    pub negative: bool,
    pub magnitude: u32,
}

/// A reset value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetValue<'src> {
    /// `reset 0x7f`
    Value(u32),
    /// `reset Disabled` — by variant name (fields only).
    Variant(&'src str),
}

/// The requirement expressed by a `requires` clause.
///
/// Entitlements are *not* a boolean expression tree — mirroring
/// `phm::entitlement::Space`, this is a list of lists: a space of [`Pattern`]s,
/// *any* of which satisfies the requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct Space<'src> {
    /// `|`-separated.
    pub patterns: Vec<Spanned<Pattern<'src>>>,
}

/// A set of entitlements, *all* of which must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern<'src> {
    /// `&`-separated paths to variants.
    pub entitlements: Vec<Spanned<Path<'src>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Import<'src> {
    pub path: Spanned<Path<'src>>,
}

/// The device's interrupt vector table, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Interrupts<'src> {
    pub entries: Vec<Spanned<InterruptEntry<'src>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterruptEntry<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub kind: InterruptKind<'src>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterruptKind<'src> {
    /// `reserved` — an unoccupied slot in the vector table.
    Reserved,
    Handler(Spanned<&'src str>),
}
