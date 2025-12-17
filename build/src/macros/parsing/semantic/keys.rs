use model::{field::Field, peripheral::Peripheral, register::Register};

macro_rules! key {
    ( $ty_name: ident, $model_component: ident, $doc: literal ) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Hash, PartialEq, Eq)]
        pub struct $ty_name(String);

        impl $ty_name {
            /// Produce the key for the provided model component.
            pub fn from_model(component: &$model_component) -> Self {
                Self(component.module_name().to_string())
            }

            /// Speculatively produce a key from the provided identifier.
            pub fn from_ident(ident: impl Into<String>) -> Self {
                Self(ident.into())
            }
        }
    };
    ( $(( $ty_name: ident, $model_component: ident, $doc: literal ) $(,)?)+ ) => {
        $(
            key! { $ty_name, $model_component, $doc }
        )+
    }
}

key! {
    (PeripheralKey, Peripheral, "The key used to query the parsed gate input for peripheral-level items."),
    (RegisterKey, Register, "The key used to query the parsed gate input for register-level items."),
    (FieldKey, Field, "The key used to query the parsed gate input for field-level items."),
}
