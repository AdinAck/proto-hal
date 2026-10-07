//! The model components related to [Peripherals](Peripheral).
//!
//! A peripheral is located at a *base address*, from which the offsets of its [registers](crate::register) are
//! measured.

use derive_more::{From, Into};
use source::{Span, Spanned};
use ters::ters;

use crate::{Leaky, Word, entitlement::Space, register::RegisterId};

/// A peripheral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peripheral {
    /// The peripheral name.
    pub name: Spanned<String>,
    /// The peripheral base address.
    pub address: Spanned<Word>,
    /// The [ontological](crate::entitlement#ontological) entitlement space, if applicable.
    pub ontological_entitlements: Option<Spanned<Space>>,
    /// Doc comments pertaining to this peripheral.
    pub docs: Vec<String>,
    /// The [leaky](crate#leaky) qualifier, if applicable.
    pub leaky: Option<Spanned<Leaky>>,
    /// The span of the declaration.
    pub span: Span,
}

/// A [`Peripheral`] node in the model.
#[ters]
#[derive(Debug, Clone)]
pub(crate) struct PeripheralNode {
    pub(crate) peripheral: Peripheral,
    /// The child registers, in the order they were added.
    #[get]
    registers: Vec<RegisterId>,
}

impl PeripheralNode {
    /// Create a node containing the provided peripheral, with no child registers.
    pub(crate) fn new(peripheral: Peripheral) -> Self {
        Self {
            peripheral,
            registers: Vec::new(),
        }
    }

    /// Add the provided register to the child registers of the peripheral.
    pub(crate) fn add_register(&mut self, register: RegisterId) {
        self.registers.push(register);
    }
}

/// A handle to a [`Peripheral`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct PeripheralId(usize);
