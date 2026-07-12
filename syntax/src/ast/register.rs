//! Registers: word-sized windows into a peripheral.

use super::{Domain, Field, FieldGroup, Head, ResetValue, Schema, Span, Spanned};

/// `register name @ offset reset value { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct Register<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    /// `leaky` — all interactions with the register's fields are `unsafe`.
    pub leaky: Option<Span>,
    /// The span of the `array` keyword.
    pub array: Option<Span>,
    pub head: Head<'src>,
    /// `@` — the offset(s) within the parent peripheral. Register arrays may
    /// auto-increment (`[0x20, ...]`): the register size is known.
    pub domain: Option<Spanned<Domain>>,
    /// The register's reset value. Fields may also specify their own; where
    /// both exist they must agree.
    pub reset: Option<Spanned<ResetValue<'src>>>,
    pub body: Option<Spanned<Vec<Spanned<RegisterItem<'src>>>>>,
}

/// What may appear within a register.
#[derive(Debug, Clone, PartialEq)]
pub enum RegisterItem<'src> {
    Field(Field<'src>),
    FieldGroup(FieldGroup<'src>),
    /// A schema placement: the schema's types manifest within this register.
    Schema(Schema<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}
