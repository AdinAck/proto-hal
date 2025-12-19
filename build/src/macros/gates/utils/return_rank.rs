use indexmap::IndexMap;
use model::{field::FieldNode, model::View, peripheral::PeripheralNode, register::RegisterNode};
use syn::Path;

use crate::macros::parsing::semantic::{
    self, FieldEntry, FieldItem, FieldKey, PeripheralKey, RegisterKey,
    policies::{self, Refine},
};

type PeripheralMap<'cx> =
    IndexMap<PeripheralKey, (Path, View<'cx, PeripheralNode>, RegisterMap<'cx>)>;
type RegisterMap<'cx> = IndexMap<RegisterKey, (View<'cx, RegisterNode>, FieldMap<'cx>)>;
type FieldMap<'cx> = IndexMap<FieldKey, View<'cx, FieldNode>>;

/// The rank of the structure to be returned from the gate.
pub enum ReturnRank<'cx> {
    /// There is no return value.
    Empty,
    /// Only one field is present.
    Field {
        peripheral_key: PeripheralKey,
        peripheral_path: Path,
        peripheral: View<'cx, PeripheralNode>,
        register_key: RegisterKey,
        register: View<'cx, RegisterNode>,
        field_key: FieldKey,
        field: View<'cx, FieldNode>,
    },
    /// Only one register is present.
    Register {
        peripheral_key: PeripheralKey,
        peripheral_path: Path,
        peripheral: View<'cx, PeripheralNode>,
        register_key: RegisterKey,
        register: View<'cx, RegisterNode>,
        fields: FieldMap<'cx>,
    },
    /// Any number of peripherals are present.
    Peripheral(PeripheralMap<'cx>),
}

impl<'cx> ReturnRank<'cx> {
    pub fn next(
        self,
        peripheral_key: PeripheralKey,
        peripheral_path: Path,
        peripheral: View<'cx, PeripheralNode>,
        register_key: RegisterKey,
        register: View<'cx, RegisterNode>,
        field_key: FieldKey,
        field: View<'cx, FieldNode>,
    ) -> Self {
        match self {
            ReturnRank::Empty => {
                // clearly the gate is not empty, there is at least one field!

                // record the peripheral, register, and field

                ReturnRank::Field {
                    peripheral_key,
                    peripheral_path,
                    peripheral,
                    register_key,
                    register,
                    field_key,
                    field,
                }
            }
            ReturnRank::Field {
                peripheral_key: existing_peripheral_key,
                peripheral_path: existing_peripheral_path,
                peripheral: existing_peripheral,
                register_key: existing_register_key,
                register: existing_register,
                field_key: existing_field_key,
                field: existing_field,
            } => {
                // the new field is in:
                // 1. the *existing* register
                // 2. a *new* peripheral and/or register

                if register_key == existing_register_key {
                    // 1. the field is in the *existing* register, promote to Kind::Register

                    ReturnRank::Register {
                        peripheral_key,
                        peripheral_path,
                        peripheral,
                        register_key,
                        register,
                        fields: IndexMap::from([
                            (existing_field_key, existing_field),
                            (field_key, field),
                        ]),
                    }
                } else {
                    // 2. the field is in a *new* peripheral and/or register, promote to Kind::Peripheral

                    let mut map = IndexMap::new();

                    // insert the *existing* peripheral, register, and field
                    map.insert(
                        existing_peripheral_key,
                        (
                            existing_peripheral_path,
                            existing_peripheral,
                            IndexMap::from([(
                                existing_register_key,
                                (
                                    existing_register,
                                    IndexMap::from([(existing_field_key, existing_field)]),
                                ),
                            )]),
                        ),
                    );

                    // insert the *new* field into the *existing* peripheral and/or register or create a *new*
                    // peripheral and/or register as needed
                    map.entry(peripheral_key)
                        .or_insert((peripheral_path, peripheral, IndexMap::new()))
                        .2
                        .entry(register_key)
                        .or_insert((register, IndexMap::from([(field_key, field)])));

                    ReturnRank::Peripheral(map)
                }
            }
            ReturnRank::Register {
                peripheral_key: existing_peripheral_key,
                peripheral_path: existing_peripheral_path,
                peripheral: existing_peripheral,
                register_key: existing_register_key,
                register: existing_register,
                fields: mut existing_fields,
            } => {
                // the new field is in:
                // 1. the *existing* register
                // 2. a *new* peripheral and/or register

                if register_key == existing_register_key {
                    // 1. the field is in the *existing* register, promote to Kind::Register

                    existing_fields.insert(field_key, field);

                    ReturnRank::Register {
                        peripheral_key: existing_peripheral_key,
                        peripheral_path: existing_peripheral_path,
                        peripheral: existing_peripheral,
                        register_key: existing_register_key,
                        register: existing_register,
                        fields: existing_fields,
                    }
                } else {
                    // 2. the field is in a *new* peripheral and/or register, promote to Kind::Peripheral

                    let mut map = IndexMap::new();

                    // insert the existing peripheral, register, and fields
                    map.insert(
                        existing_peripheral_key,
                        (
                            existing_peripheral_path,
                            existing_peripheral,
                            IndexMap::from([(
                                existing_register_key,
                                (existing_register, existing_fields), // this line is the only difference from the
                                                                      // above arm
                            )]),
                        ),
                    );

                    // insert the *new* field into the *existing* peripheral and/or register or create a *new*
                    // peripheral and/or register as needed
                    map.entry(peripheral_key)
                        .or_insert((peripheral_path, peripheral, IndexMap::new()))
                        .2
                        .entry(register_key)
                        .or_insert((register, IndexMap::from([(field_key, field)])));

                    ReturnRank::Peripheral(map)
                }
            }
            ReturnRank::Peripheral(mut map) => {
                // the new field is in:
                // 1. an *existing* peripheral and/or register
                // 2. a *new* peripheral and/or register

                // insert the *new* field into an *existing* peripheral and/or register or create a *new*
                // peripheral and/or register as needed
                map.entry(peripheral_key)
                    .or_insert((peripheral_path, peripheral, IndexMap::new()))
                    .2
                    .entry(register_key)
                    .or_insert((register, IndexMap::from([(field_key, field)])));

                ReturnRank::Peripheral(map)
            }
        }
    }

    /// Parse the return rank from the input in *strict* mode.
    /// Fields to be considered for the rank must be explicitly specified.
    pub fn from_input_strict<EntryPolicy>(
        input: &'cx semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy>,
        filter: impl Fn(&FieldItem<'cx, EntryPolicy>) -> bool,
    ) -> Self
    where
        EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
    {
        let mut rank = ReturnRank::Empty;

        for (peripheral_key, peripheral_item) in input.peripherals() {
            for (register_key, register_item) in peripheral_item.registers() {
                for (field_key, field_item) in register_item.fields() {
                    if filter(field_item) {
                        rank = rank.next(
                            peripheral_key.clone(),
                            peripheral_item.path().clone(),
                            peripheral_item.peripheral().clone(),
                            register_key.clone(),
                            register_item.register().clone(),
                            field_key.clone(),
                            field_item.field().clone(),
                        );
                    }
                }
            }
        }

        rank
    }

    /// Parse the return rank from the input in *relaxed* mode.
    /// When an item has no children, all fields within the corresponding model component
    /// will be considered for the rank.
    pub fn from_input_relaxed<EntryPolicy>(
        input: &'cx semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy>,
        filter: impl Fn(&View<'cx, FieldNode>) -> bool,
    ) -> Self
    where
        EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
    {
        let mut rank = ReturnRank::Empty;

        for (peripheral_key, peripheral_item) in input.peripherals() {
            if peripheral_item.registers().is_empty() {
                // fill unspecified registers/fields
                for register in peripheral_item.peripheral().registers() {
                    for field in register.fields() {
                        if filter(&field) {
                            rank = rank.next(
                                PeripheralKey::from_model(peripheral_item.peripheral()),
                                peripheral_item.path().clone(),
                                peripheral_item.peripheral().clone(),
                                RegisterKey::from_model(&register),
                                register.clone(),
                                FieldKey::from_model(&field),
                                field,
                            );
                        }
                    }
                }
            }

            for (register_key, register_item) in peripheral_item.registers() {
                if register_item.fields().is_empty() {
                    // fill unspecified fields
                    for field in register_item.register().fields() {
                        if filter(&field) {
                            rank = rank.next(
                                PeripheralKey::from_model(peripheral_item.peripheral()),
                                peripheral_item.path().clone(),
                                peripheral_item.peripheral().clone(),
                                RegisterKey::from_model(register_item.register()),
                                register_item.register().clone(),
                                FieldKey::from_model(&field),
                                field,
                            );
                        }
                    }
                }

                for (field_key, field_item) in register_item.fields() {
                    if filter(field_item.field()) {
                        rank = rank.next(
                            peripheral_key.clone(),
                            peripheral_item.path().clone(),
                            peripheral_item.peripheral().clone(),
                            register_key.clone(),
                            register_item.register().clone(),
                            field_key.clone(),
                            field_item.field().clone(),
                        );
                    }
                }
            }
        }

        rank
    }
}
