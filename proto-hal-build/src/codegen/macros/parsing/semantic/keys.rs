use ir::structures::{field::Field, peripheral::Peripheral, register::Register};

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PeripheralKey(String);

impl PeripheralKey {
    pub fn from_model(peripheral: &Peripheral) -> Self {
        Self(peripheral.module_name().to_string())
    }

    pub fn from_ident(ident: impl Into<String>) -> Self {
        Self(ident.into())
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct RegisterKey((String, String));

impl RegisterKey {
    pub fn from_model(peripheral: &Peripheral, register: &Register) -> Self {
        Self((
            peripheral.module_name().to_string(),
            register.module_name().to_string(),
        ))
    }

    pub fn from_ident(
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
    ) -> Self {
        Self((peripheral_ident.into(), register_ident.into()))
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct FieldKey(String);

impl FieldKey {
    pub fn from_model(field: &Field) -> Self {
        Self(field.module_name().to_string())
    }

    pub fn from_ident(ident: impl Into<String>) -> Self {
        Self(ident.into())
    }
}
