use ir::structures::{field::Field, peripheral::Peripheral, register::Register};

/// The key used to query the parsed gate input for peripheral-level items.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PeripheralKey(String);

impl PeripheralKey {
    /// Produce the key for the provided peripheral model element.
    pub fn from_model(peripheral: &Peripheral) -> Self {
        Self(peripheral.module_name().to_string())
    }

    /// Speculatively produce a key from the provided identifier.
    pub fn from_ident(ident: impl Into<String>) -> Self {
        Self(ident.into())
    }
}

/// The key used to query the parsed gate input for register-level items.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RegisterKey((String, String));

impl RegisterKey {
    /// Produce the key for the provided peripheral and register model elements.
    pub fn from_model(peripheral: &Peripheral, register: &Register) -> Self {
        Self((
            peripheral.module_name().to_string(),
            register.module_name().to_string(),
        ))
    }

    /// Speculatively produce a key from the provided identifiers.
    pub fn from_ident(
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
    ) -> Self {
        Self((peripheral_ident.into(), register_ident.into()))
    }
}

/// The key used to query the parsed gate input for field-level items.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct FieldKey(String);

impl FieldKey {
    /// Produce the key for the provided field model element.
    pub fn from_model(field: &Field) -> Self {
        Self(field.module_name().to_string())
    }

    /// Speculatively produce a key from the provided identifier.
    pub fn from_ident(ident: impl Into<String>) -> Self {
        Self(ident.into())
    }
}
