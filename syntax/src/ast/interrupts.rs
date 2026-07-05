//! The device's interrupt vector table.

use super::Spanned;

/// `interrupts { ... }` — the vector table, in order.
///
/// Positions are implied by order; unoccupied slots are written `reserved`.
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
    /// A named interrupt handler.
    Handler(Spanned<&'src str>),
}
