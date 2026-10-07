//! The model components related to [Registers](Register).
//!
//! A register is the smallest unit of MMIO transaction, located at an offset from the parent
//! [peripheral](crate::peripheral) base address. Registers host [fields](crate::field).

use source::{Span, Spanned};

use crate::{field::FieldId, peripheral::PeripheralId};

/// A register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    /// The register name.
    pub name: Spanned<String>,
    /// The offset from the parent peripheral base address.
    pub offset: Spanned<u32>,
    /// The reset value, if applicable.
    pub reset: Option<Spanned<u32>>,
    /// Doc comments pertaining to this register.
    pub docs: Vec<String>,
    /// Whether the register is [leaky](crate#leaky).
    pub leaky: bool,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Register`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegisterId(pub(crate) usize);

/// A [`Register`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct RegisterNode {
    pub(crate) register: Register,
    pub(crate) peripheral: PeripheralId,
    pub(crate) fields: Vec<FieldId>,
}
