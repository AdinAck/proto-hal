//! Groups: named collections of like members.
//!
//! A group is *not* a container in the elaboration sense: its members remain
//! children of the group's parent. Groups exist at each hardware level —
//! `peripheral group`, `register group`, `field group` — and do not nest.

use super::{Field, Peripheral, Register, Schema, Spanned};

/// `<kind> group name { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'src, T> {
    pub docs: Vec<Spanned<&'src str>>,
    pub name: Option<Spanned<&'src str>>,
    pub body: Spanned<Vec<Spanned<GroupItem<'src, T>>>>,
}

/// What may appear within a group: members of its level, and schema
/// placements.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupItem<'src, T> {
    Member(T),
    /// A schema placement: the schema's types manifest within this group.
    Schema(Schema<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}

pub type PeripheralGroup<'src> = Group<'src, Peripheral<'src>>;
pub type RegisterGroup<'src> = Group<'src, Register<'src>>;
pub type FieldGroup<'src> = Group<'src, Field<'src>>;
