//! The model components related to [Peripherals](Peripheral).
//!
//! A peripheral is located at a *base address*, from which the offsets of its [registers](crate::register) are
//! measured.

use derive_more::{From, Into};
use source::{Span, Spanned};

use crate::{Word, register::RegisterId};

/// A peripheral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peripheral {
    /// The peripheral name.
    pub name: Spanned<String>,
    /// The peripheral base address.
    pub address: Spanned<Word>,
    /// Doc comments pertaining to this peripheral.
    pub docs: Vec<String>,
    /// Whether the peripheral is [leaky](crate#leaky).
    pub leaky: bool,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Peripheral`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct PeripheralId(usize);

/// A [`Peripheral`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct PeripheralNode {
    pub(crate) peripheral: Peripheral,
    pub(crate) registers: Vec<RegisterId>,
}

impl PeripheralNode {
    /// Create a node containing the provided peripheral, with no child registers.
    pub(crate) fn new(peripheral: Peripheral) -> Self {
        Self {
            peripheral,
            registers: Vec::new(),
        }
    }
}
