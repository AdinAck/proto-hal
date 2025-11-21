use indexmap::IndexMap;
use syn::Ident;

use crate::codegen::macros::parsing::semantic::{
    self, FieldEntry, FieldItem, FieldKey, RegisterItem, RegisterKey,
    policies::{self, Refine},
};

type RegisterMap<'cx, EntryPolicy> = IndexMap<
    &'cx RegisterKey,
    (
        &'cx RegisterItem<'cx, EntryPolicy>,
        IndexMap<&'cx FieldKey, &'cx FieldItem<'cx, EntryPolicy>>,
    ),
>;

/// The rank of the structure to be returned from the gate.
pub enum ReturnRank<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    /// There is no return value.
    Empty,
    /// Only one field is present.
    Field {
        peripheral_key: Ident,
        register_key: &'cx RegisterKey,
        register_item: &'cx RegisterItem<'cx, EntryPolicy>,
        field_key: &'cx FieldKey,
        field_item: &'cx FieldItem<'cx, EntryPolicy>,
    },
    /// Only one register is present.
    Register {
        peripheral_key: Ident,
        register_key: &'cx RegisterKey,
        register_item: &'cx RegisterItem<'cx, EntryPolicy>,
        fields: IndexMap<&'cx FieldKey, &'cx FieldItem<'cx, EntryPolicy>>,
    },
    /// Any number of peripherals are present.
    Peripheral(IndexMap<Ident, RegisterMap<'cx, EntryPolicy>>),
}

impl<'cx, EntryPolicy> ReturnRank<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    pub fn next(
        self,
        peripheral_key: Ident,
        register_key: &'cx RegisterKey,
        register_item: &'cx RegisterItem<'cx, EntryPolicy>,
        field_key: &'cx FieldKey,
        field_item: &'cx FieldItem<'cx, EntryPolicy>,
    ) -> Self {
        match self {
            ReturnRank::Empty => {
                // clearly the gate is not empty, there is at least one field!

                // record the peripheral, register, and field

                ReturnRank::Field {
                    peripheral_key,
                    register_key,
                    register_item,
                    field_key,
                    field_item,
                }
            }
            ReturnRank::Field {
                peripheral_key: existing_peripheral_key,
                register_key: existing_register_key,
                register_item: existing_register_item,
                field_key: existing_field_key,
                field_item: existing_field_item,
            } => {
                // the gate has more than one field, potentially even more than one register

                if register_key == existing_register_key {
                    // if the field is in the same register, promote to Kind::Register

                    ReturnRank::Register {
                        peripheral_key,
                        register_key,
                        register_item,
                        fields: IndexMap::from([
                            (existing_field_key, existing_field_item),
                            (field_key, field_item),
                        ]),
                    }
                } else {
                    // if the field is in a different register, promote to Kind::Peripheral

                    let mut map = IndexMap::new();

                    // insert the existing peripheral, register, and field
                    map.insert(
                        existing_peripheral_key,
                        IndexMap::from([(
                            existing_register_key,
                            (
                                existing_register_item,
                                IndexMap::from([(existing_field_key, existing_field_item)]),
                            ),
                        )]),
                    );

                    // insert the new peripheral (or don't if they are the same) and insert the new
                    // register and field
                    map.entry(peripheral_key)
                        .or_insert_with(IndexMap::new)
                        .insert(
                            register_key,
                            (register_item, IndexMap::from([(field_key, field_item)])),
                        );

                    ReturnRank::Peripheral(map)
                }
            }
            ReturnRank::Register {
                peripheral_key: existing_peripheral_key,
                register_key: existing_register_key,
                register_item: existing_register_item,
                fields: mut existing_fields,
            } => {
                // the gate could have more than one register, or not

                if register_key == existing_register_key {
                    // if the field is in the existing register, stay in Kind::Register

                    existing_fields.insert(field_key, field_item);

                    ReturnRank::Register {
                        peripheral_key: existing_peripheral_key,
                        register_key: existing_register_key,
                        register_item: existing_register_item,
                        fields: existing_fields,
                    }
                } else {
                    // if the field is in a new register, promote to Kind::Peripheral

                    let mut map = IndexMap::new();

                    // insert the existing peripheral, register, and fields
                    map.insert(
                        existing_peripheral_key,
                        IndexMap::from([(existing_register_key, (register_item, existing_fields))]),
                    );

                    // insert the new peripheral (or don't if they are the same) and insert the new
                    // register and fields
                    map.entry(peripheral_key)
                        .or_insert_with(IndexMap::new)
                        .insert(
                            register_key,
                            (register_item, IndexMap::from([(field_key, field_item)])),
                        );

                    ReturnRank::Peripheral(map)
                }
            }
            ReturnRank::Peripheral(mut map) => {
                // the gate could have more than one peripheral, or not

                // peripheral exists
                let Some(existing_registers) = map.get_mut(&peripheral_key) else {
                    map.insert(
                        peripheral_key,
                        IndexMap::from([(
                            register_key,
                            (register_item, IndexMap::from([(field_key, field_item)])),
                        )]),
                    );

                    return ReturnRank::Peripheral(map);
                };

                // register exists
                let Some((.., existing_fields)) = existing_registers.get_mut(register_key) else {
                    existing_registers.insert(
                        register_key,
                        (register_item, IndexMap::from([(field_key, field_item)])),
                    );

                    return ReturnRank::Peripheral(map);
                };

                existing_fields.insert(field_key, field_item);

                ReturnRank::Peripheral(map)
            }
        }
    }

    pub fn from_input(
        input: &'cx semantic::Gate<'cx, policies::peripheral::ForbidPath, EntryPolicy>,
        filter: impl Fn(&FieldItem<'cx, EntryPolicy>) -> bool,
    ) -> Self {
        let mut rank = ReturnRank::Empty;

        for (register_key, register_item) in input.registers() {
            for (field_key, field_item) in register_item.fields() {
                if filter(field_item) {
                    rank = rank.next(
                        register_item.peripheral().module_name(),
                        register_key,
                        register_item,
                        field_key,
                        field_item,
                    );
                }
            }
        }

        rank
    }
}
