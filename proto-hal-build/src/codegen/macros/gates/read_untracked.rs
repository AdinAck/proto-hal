use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};
use ir::structures::hal::Hal;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Ident};

use crate::codegen::macros::{
    diagnostic::{Diagnostic, Diagnostics},
    gates::{
        fragments::{read_value_expr, read_value_ty, register_address},
        utils::{render_diagnostics, suggestions, unique_register_ident},
    },
    parsing::{
        semantic::{
            self, FieldItem, FieldKey, RegisterItem, RegisterKey,
            policies::{ForbidEntry, ForbidPeripherals},
        },
        syntax::Override,
    },
};

type EntryPolicy = ForbidEntry;
type Input<'cx> = semantic::Gate<'cx, ForbidPeripherals, EntryPolicy>;

pub fn read_untracked(model: &Hal, tokens: TokenStream) -> TokenStream {
    let args = match syn::parse2(tokens) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let (input, mut diagnostics) = Input::parse(&args, model);
    diagnostics.extend(validate(&input));

    let mut overridden_base_addrs: HashMap<Ident, Expr> = HashMap::new();

    for override_ in &args.overrides {
        match override_ {
            Override::BaseAddress(ident, expr) => {
                overridden_base_addrs.insert(ident.clone(), expr.clone());
            }
            Override::CriticalSection(expr) => diagnostics.push(
                syn::Error::new_spanned(
                    &expr,
                    "stand-alone read access is atomic and doesn't require a critical section",
                )
                .into(),
            ),
            Override::Unknown(ident) => diagnostics.push(
                syn::Error::new_spanned(&ident, format!("unexpected override \"{}\"", ident))
                    .into(),
            ),
        };
    }

    let suggestions = suggestions(&args, &diagnostics);
    let errors = render_diagnostics(diagnostics);

    let mut reg_idents = Vec::new();
    let mut addrs = Vec::new();
    let mut read_values = Vec::new();

    for register_item in input.visit_registers() {
        let register_path = register_item.path();

        reg_idents.push(unique_register_ident(
            register_item.peripheral(),
            register_item.register(),
        ));
        addrs.push(register_address(
            register_item.peripheral(),
            register_item.register(),
            &overridden_base_addrs,
        ));

        for field_item in register_item.fields().values() {
            if let Some(read) = field_item.field().access.get_read() {
                read_values.push(read_value_expr(
                    &register_path,
                    field_item.ident(),
                    register_item.peripheral(),
                    register_item.register(),
                    field_item.field(),
                ));
            }
        }
    }

    quote! {
        #suggestions
        #errors

        {
            unsafe fn gate() -> #return_ty {
                #(
                    let #reg_idents = unsafe {
                        ::core::ptr::read_volatile(#addrs as *const u32)
                    };
                )*

                // todo...
            }

            gate()
        }
    }
}

fn validate<'cx>(input: &Input<'cx>) -> Diagnostics {
    input
        .visit_fields()
        .filter_map(|field_item| {
            if !field_item.field().access.is_read() {
                Some(Diagnostic::field_must_be_readable(field_item.ident()))
            } else {
                None
            }
        })
        .collect()
}

fn make_return_ty<'cx>(rank: &ReturnRank<'cx>) -> Option<TokenStream> {
    match rank {
        ReturnRank::Empty => None,
        ReturnRank::Field {
            register_item,
            field_item,
            ..
        } => Some(read_value_ty(
            &register_item.path(),
            field_item.ident(),
            &field_item.field().access.get_read()?.numericity,
        )),
        ReturnRank::Register {
            peripheral_key,
            register_key,
            register_item,
            fields,
        } => todo!(),
        ReturnRank::Peripheral(index_map) => todo!(),
    }
}

fn get_return_rank<'cx>(input: &'cx Input<'cx>) -> ReturnRank<'cx> {
    let mut rank = ReturnRank::Empty;

    for (register_key, register_item) in input.registers() {
        for (field_key, field_item) in register_item.fields() {
            rank = rank.next(
                register_item.peripheral().module_name(),
                register_key,
                register_item,
                field_key,
                field_item,
            );
        }
    }

    rank
}

/// The rank of the structure to be returned from the gate.
enum ReturnRank<'cx> {
    /// The gate is empty.
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
    Peripheral(
        IndexMap<
            Ident,
            IndexMap<
                &'cx RegisterKey,
                (
                    &'cx RegisterItem<'cx, EntryPolicy>,
                    IndexMap<&'cx FieldKey, &'cx FieldItem<'cx, EntryPolicy>>,
                ),
            >,
        >,
    ),
}

impl<'cx> ReturnRank<'cx> {
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
}
