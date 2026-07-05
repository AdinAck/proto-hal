//! Declaration trees: whole-structure model construction.
//!
//! A declaration is plain data describing an item and everything within it.
//! Handing a complete tree to [`Composition::insert_peripheral`] (or
//! [`insert_peripheral_group`](Composition::insert_peripheral_group)) performs
//! all of the bookkeeping — index wiring, duplicate detection, numericity
//! construction — in one step.
//!
//! Entitlements and schema references are deliberately *absent* from
//! declarations: they reach across the whole device, so they are registered
//! afterwards — once every structure exists — via [`Composition::entitle`],
//! [`Composition::link_field`], and [`Composition::extend_field`]. This makes
//! model construction two-phase (structure, then references) and therefore
//! independent of declaration order.
//!
//! This is the successor to the [`Entry`](crate::model::Entry) interfaces,
//! which interleave structure and references and consequently impose ordering
//! requirements on their callers.
//!
//! [`Composition::insert_peripheral`]: crate::Composition::insert_peripheral
//! [`Composition::entitle`]: crate::Composition::entitle

use crate::{Field, Peripheral, Register, Variant, field::access::Access};

/// A peripheral and everything within it.
#[derive(Debug, Clone)]
pub struct PeripheralDecl {
    pub peripheral: Peripheral,
    pub registers: Vec<RegisterDecl>,
    pub register_groups: Vec<GroupDecl<RegisterDecl>>,
}

/// A register and everything within it.
#[derive(Debug, Clone)]
pub struct RegisterDecl {
    pub register: Register,
    pub fields: Vec<FieldDecl>,
    pub field_groups: Vec<GroupDecl<FieldDecl>>,
}

/// A field, its access modality, and its variants.
///
/// The access carries *empty* numericities: variants are declared here and
/// composed into the numericities during insertion.
///
/// A field assuming a schema is declared with placeholder access and no
/// variants, then [linked](crate::Composition::link_field) once every schema
/// exists.
#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub field: Field,
    pub access: Access,
    pub variants: Vec<VariantDecl>,
}

/// A schema: a shareable variant vocabulary, manifesting at its placement.
///
/// Schemas have no access modality — each variant's side decides which
/// numericities it enters, and referencing fields project the vocabulary
/// onto their own modality.
#[derive(Debug, Clone)]
pub struct SchemaDecl {
    pub ident: String,
    pub variants: Vec<VariantDecl>,
    pub docs: Vec<String>,
}

/// A variant and the numericity it occupies within its container.
#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub variant: Variant,
    pub side: Option<Side>,
}

/// Which numericity of a `read write` access a variant occupies. Unsided
/// variants occupy every numericity of their container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Read,
    Write,
}

/// A named group of like members.
#[derive(Debug, Clone)]
pub struct GroupDecl<T> {
    pub name: String,
    pub members: Vec<T>,
}
