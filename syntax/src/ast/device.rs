//! The root of elaboration.

use super::{Head, Interrupts, Peripheral, PeripheralGroup, Schema, Spanned};

/// `device name { ... }` — the single structure a model description elaborates.
///
/// The entry file must define exactly one device; everything it contains is
/// *corporeal*, elaborated into the model.
#[derive(Debug, Clone, PartialEq)]
pub struct Device<'src> {
    pub docs: Vec<Spanned<&'src str>>,
    pub head: Head<'src>,
    pub body: Option<Spanned<Vec<Spanned<DeviceItem<'src>>>>>,
}

/// What may appear within a device.
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceItem<'src> {
    Peripheral(Peripheral<'src>),
    PeripheralGroup(PeripheralGroup<'src>),
    /// A schema placement: gives the schema a location in the device so that
    /// fields may [assume](super::Field::assumes) it.
    Schema(Schema<'src>),
    Interrupts(Interrupts<'src>),
    /// A region that failed to parse. The error has already been reported;
    /// elaboration skips these.
    Error,
}
