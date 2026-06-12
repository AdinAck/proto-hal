use proc_macro2::TokenStream;

use crate::{
    Model,
    field::{FieldIndex, access::Access},
    group::{FieldGroupIndex, PeripheralGroupIndex, RegisterGroupIndex},
    peripheral::PeripheralIndex,
    register::RegisterIndex,
};

#[derive(Debug, Clone)]
pub enum Source {
    Inherent(Access),
    Linked { parent: ParentIndex, access: Access },
}

impl Source {
    /// Get the [`Access`] of the field whether it is [`inherent`](Source::Inherent) or [`linked`](Source::Linked).
    pub fn access(&self) -> &Access {
        match self {
            Self::Inherent(access) => access,
            Self::Linked { access, .. } => access,
        }
    }

    /// Get an [`inherent`](Source::Inherent) [`Access`] or [`None`].
    pub fn inherent(&self) -> Option<&Access> {
        match self {
            Self::Inherent(access) => Some(access),
            Self::Linked { .. } => None,
        }
    }

    /// [`inherent`](Source::inherent) but mutable.
    pub(crate) fn inherent_mut(&mut self) -> Option<&mut Access> {
        match self {
            Self::Inherent(access) => Some(access),
            Self::Linked { .. } => None,
        }
    }

    /// Get a [`linked`](Source::Linked) [`Access`] or [`None`].
    pub fn linked(&self) -> Option<&Access> {
        match self {
            Self::Linked { access, .. } => Some(access),
            Self::Inherent(..) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParentIndex {
    Peripheral(PeripheralIndex),
    PeripheralGroup(PeripheralGroupIndex),
    Register(RegisterIndex),
    RegisterGroup(RegisterGroupIndex),
    Field(FieldIndex),
    FieldGroup(FieldGroupIndex),
}

impl ParentIndex {
    pub fn path(self, model: &Model) -> TokenStream {
        match self {
            ParentIndex::Peripheral(index) => model.get_peripheral(index).path(),
            ParentIndex::PeripheralGroup(index) => model.get_peripheral_group(index).path(),
            ParentIndex::Register(index) => model.get_register(index).path(),
            ParentIndex::RegisterGroup(index) => model.get_register_group(index).path(),
            ParentIndex::Field(index) => model.get_field(index).path(),
            ParentIndex::FieldGroup(index) => model.get_field_group(index).path(),
        }
    }
}
