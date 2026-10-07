//! The model components related to [Peripherals](Peripheral).
//!
//! A peripheral is located at a *base address*, denoting the start of its [register](crate::register) address space.

use source::{Span, Spanned};

use crate::register::RegisterId;

/// A peripheral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peripheral {
    /// The peripheral name.
    pub name: Spanned<String>,
    /// The peripheral base address.
    pub address: Spanned<u32>,
    /// Doc comments pertaining to this peripheral.
    pub docs: Vec<String>,
    /// Whether the peripheral is [leaky](crate#leaky).
    pub leaky: bool,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Peripheral`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeripheralId(pub(crate) usize);

/// A [`Peripheral`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct PeripheralNode {
    pub(crate) peripheral: Peripheral,
    pub(crate) registers: Vec<RegisterId>,
}
