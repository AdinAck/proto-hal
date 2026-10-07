//! The model components related to [Registers](Register).
//!
//! A register is located at an offset from the parent [peripheral](crate::peripheral) base address, and defines exactly
//! which MMIO transactions may access the bytes it occupies, through the register [access](#access) widths.
//!
//! Registers host [fields](crate::field), the smallest units of MMIO interaction. Fields are read and written by
//! transactions on the hosting register.
//!
//! # Access
//!
//! A register's *access* is the set of widths, in bytes, at which it may be accessed. A register without access widths
//! is accessed at the [word size](crate#word-size) alone.

use derive_more::{From, Into};
use source::{Span, Spanned};

use crate::{Word, field::FieldId, peripheral::PeripheralId};

/// A register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    /// The register name.
    pub name: Spanned<String>,
    /// The offset from the parent peripheral base address.
    pub offset: Spanned<Word>,
    /// The reset value, if applicable.
    pub reset: Option<Spanned<Word>>,
    /// The register [access](crate::register#access) widths, in bytes.
    pub access: Vec<Spanned<Word>>,
    /// Doc comments pertaining to this register.
    pub docs: Vec<String>,
    /// Whether the register is [leaky](crate#leaky).
    pub leaky: bool,
    /// The span of the declaration.
    pub span: Span,
}

/// A handle to a [`Register`] in a [`Model`](crate::Model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, From, Into)]
pub struct RegisterId(usize);

/// A [`Register`] node in the model.
#[derive(Debug, Clone)]
pub(crate) struct RegisterNode {
    pub(crate) register: Register,
    pub(crate) peripheral: PeripheralId,
    pub(crate) fields: Vec<FieldId>,
}

impl RegisterNode {
    /// Create a node containing the provided register as a child of the provided peripheral, with no child fields.
    pub(crate) fn new(register: Register, peripheral: PeripheralId) -> Self {
        Self {
            register,
            peripheral,
            fields: Vec::new(),
        }
    }
}
