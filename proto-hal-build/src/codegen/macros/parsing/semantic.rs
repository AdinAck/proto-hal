//! Structures related to semantic parsing. These structures are parsed against the device model, corresponding
//! gate inputs to model elements as well as providing semantic querying.

mod entry;
mod keys;
pub mod policies;
mod transition;
mod utils;

use indexmap::IndexMap;
use model::structures::{
    field::FieldNode,
    model::{Model, View},
    peripheral::PeripheralNode,
    register::RegisterNode,
};
use syn::{Ident, Path, parse_quote};
use ters::ters;

use crate::codegen::macros::{
    diagnostic::Diagnostics,
    parsing::{
        semantic::{entry::PeripheralEntry, policies::Refine, utils::parse_peripheral},
        syntax,
    },
};

pub use entry::FieldEntry;
pub use keys::*;
pub use transition::Transition;

type PeripheralMap<'cx, EntryPolicy> = IndexMap<PeripheralKey, PeripheralItem<'cx, EntryPolicy>>;
type RegisterMap<'cx, EntryPolicy> = IndexMap<RegisterKey, RegisterItem<'cx, EntryPolicy>>;
type FieldMap<'cx, EntryPolicy> = IndexMap<FieldKey, FieldItem<'cx, EntryPolicy>>;

/// The semantically parsed gate input, with corresponding model elements.
#[ters]
pub struct Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>
where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    #[get]
    peripherals: PeripheralMap<'cx, PeripheralEntryPolicy>,
    #[get]
    registers: RegisterMap<'cx, FieldEntryPolicy>,
}

impl<'cx, PeripheralEntryPolicy, FieldEntryPolicy>
    Gate<'cx, PeripheralEntryPolicy, FieldEntryPolicy>
where
    PeripheralEntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
    FieldEntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    /// Parse the gate input against the model to produce a semantic gate input.
    pub fn parse(args: &'cx syntax::Gate, model: &'cx Model) -> (Self, Diagnostics) {
        let mut diagnostics = Diagnostics::new();
        let mut peripherals = Default::default();
        let mut registers = Default::default();

        for tree in &args.trees {
            if let Err(e) = parse_peripheral::<PeripheralEntryPolicy, _>(
                model,
                &mut peripherals,
                &mut registers,
                tree,
            ) {
                diagnostics.extend(e);
            }
        }

        (
            Self {
                peripherals,
                registers,
            },
            diagnostics,
        )
    }

    /// Query for a peripheral-level item with the provided identifier.
    pub fn get_peripheral(
        &self,
        ident: impl Into<String>,
    ) -> Option<&PeripheralItem<'cx, PeripheralEntryPolicy>> {
        self.peripherals.get(&PeripheralKey::from_ident(ident))
    }

    /// Visit all peripheral-level items.
    pub fn visit_peripherals(
        &self,
    ) -> impl Iterator<Item = &PeripheralItem<'cx, PeripheralEntryPolicy>> {
        self.peripherals.values()
    }

    /// Query for a register-level item with the provided peripheral and register identifiers.
    pub fn get_register(
        &self,
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
    ) -> Option<&RegisterItem<'cx, FieldEntryPolicy>> {
        self.registers
            .get(&RegisterKey::from_ident(peripheral_ident, register_ident))
    }

    /// Visit all register-level items.
    pub fn visit_registers(&self) -> impl Iterator<Item = &RegisterItem<'cx, FieldEntryPolicy>> {
        self.registers.values()
    }

    /// Query for a field-level item with the provided peripheral, register, and field identifiers.
    pub fn get_field(
        &self,
        peripheral_ident: impl Into<String>,
        register_ident: impl Into<String>,
        field_ident: impl Into<String>,
    ) -> Option<(
        &RegisterItem<'cx, FieldEntryPolicy>,
        &FieldItem<'cx, FieldEntryPolicy>,
    )> {
        self.registers
            .get(&RegisterKey::from_ident(peripheral_ident, register_ident))
            .and_then(|register_item| {
                register_item
                    .fields
                    .get(&FieldKey::from_ident(field_ident))
                    .map(|field_item| (register_item, field_item))
            })
    }

    /// Visit all field-level items.
    pub fn visit_fields(&self) -> impl Iterator<Item = &FieldItem<'cx, FieldEntryPolicy>> {
        self.registers
            .values()
            .flat_map(|register_item| register_item.fields.values())
    }
}

/// A peripheral-level item present in the gate.
///
/// *Note: This item DOES NOT contain child registers, and DOES contain a binding.*
#[ters]
pub struct PeripheralItem<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = PeripheralEntry<'cx>>,
{
    #[get]
    path: Path,
    #[get]
    ident: &'cx Ident,
    #[get]
    peripheral: View<'cx, PeripheralNode>,
    #[get]
    entry: EntryPolicy,
}

/// A register-level item present in the gate.
///
/// *Note: This item DOES contain child fields, and DOES NOT contain a binding.*
#[ters]
pub struct RegisterItem<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    #[get]
    peripheral_path: Path,
    #[get]
    ident: &'cx Ident,
    #[get]
    peripheral: View<'cx, PeripheralNode>,
    #[get]
    register: View<'cx, RegisterNode>,
    #[get]
    fields: FieldMap<'cx, EntryPolicy>,
}

/// A field-level item present in the gate.
///
/// *Note: This item has no children, and DOES contain a binding.*
#[ters]
pub struct FieldItem<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    #[get]
    ident: &'cx Ident,
    #[get]
    field: View<'cx, FieldNode>,
    #[get]
    entry: EntryPolicy,
}

impl<'cx, EntryPolicy> RegisterItem<'cx, EntryPolicy>
where
    EntryPolicy: Refine<'cx, Input = FieldEntry<'cx>>,
{
    /// The path to the register including segments *before* the peripheral.
    pub fn path(&self) -> Path {
        let peripheral_path = self.peripheral_path();
        let ident = self.ident();
        parse_quote! { #peripheral_path::#ident }
    }
}

#[cfg(test)]
mod tests {
    use model::structures::{
        field::Field, model::Model, peripheral::Peripheral, register::Register,
    };
    use quote::quote;
    use syn::{Ident, Path, parse_quote};

    use crate::codegen::macros::{
        diagnostic,
        parsing::{
            semantic::{Gate, policies},
            syntax,
        },
    };

    #[test]
    fn single_peripheral() {
        let peripheral_name = "foo";
        let peripheral_path = parse_quote! { ::external::foo };
        let peripheral_binding = quote! { some_foo };

        let mut model = Model::new();
        model.add_peripheral(Peripheral::new(peripheral_name, 0));

        let tokens = quote! {
            #peripheral_path(#peripheral_binding)
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactic parsing should succeed");
        let (gate, e) =
            Gate::<policies::peripheral::ConsumeOnly, policies::field::ForbidEntry>::parse(
                &args, &model,
            );

        assert!(e.is_empty(), "semantic parsing should succeed");

        let peripheral = gate
            .get_peripheral(peripheral_name)
            .expect("peripheral should exist");

        assert_eq!(peripheral.path(), &peripheral_path);
        assert_eq!(
            **peripheral.entry(),
            &syn::parse2::<syntax::Binding>(quote! { #peripheral_binding }).unwrap()
        );
        assert_eq!(peripheral.peripheral().ident, peripheral_name);
    }

    #[test]
    fn multiple_peripherals() {
        let peripheral0_name = "foo";
        let peripheral0_path = parse_quote! { ::external::foo };
        let peripheral0_binding = quote! { some_foo };
        let peripheral1_name = "bar";
        let peripheral1_path = parse_quote! { external::stuff::bar };

        let mut model = Model::new();
        model.add_peripheral(Peripheral::new(peripheral0_name, 0));
        model.add_peripheral(Peripheral::new(peripheral1_name, 0));

        let tokens = quote! {
            #peripheral0_path(#peripheral0_binding),
            #peripheral1_path,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactic parsing should succeed");
        let (gate, e) =
            Gate::<policies::peripheral::ConsumeOnly, policies::field::ForbidEntry>::parse(
                &args, &model,
            );

        assert!(e.is_empty(), "semantic parsing should succeed");

        let peripheral0 = gate
            .get_peripheral(peripheral0_name)
            .expect("peripheral should exist");

        let peripheral1 = gate
            .get_peripheral(peripheral1_name)
            .expect("peripheral should exist");

        assert_eq!(peripheral0.path(), &peripheral0_path);
        assert_eq!(
            **peripheral0.entry(),
            &syn::parse2::<syntax::Binding>(quote! { #peripheral0_binding }).unwrap()
        );
        assert_eq!(peripheral0.peripheral().ident, peripheral0_name);

        assert_eq!(peripheral1.path(), &peripheral1_path);
        assert_eq!(peripheral1.peripheral().ident, peripheral1_name);
    }

    #[test]
    fn single_register() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };

        let mut model = Model::new();
        model
            .add_peripheral(Peripheral::new(peripheral_name, 0))
            .add_register(Register::new(register_name, 0));

        let tokens = quote! {
            #peripheral_path::#register_ident
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactic parsing should succeed");
        let (.., e) =
            Gate::<policies::peripheral::ForbidPath, policies::field::RequireBinding>::parse(
                &args, &model,
            );

        assert!(
            e.iter().any(|diagnostic| matches!(
                diagnostic.kind(),
                diagnostic::Kind::UnexpectedRegister
            ))
        )
    }

    #[test]
    fn field_must_be_writable() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };
        let field_name = "baz";
        let field_ident: Ident = parse_quote! { baz };

        let mut model = Model::new();
        model
            .add_peripheral(Peripheral::new(peripheral_name, 0))
            .add_register(Register::new(register_name, 0))
            .add_read_field(Field::new(field_name, 0, 0));

        let tokens = quote! {
            #peripheral_path::#register_ident::#field_ident(&mut my_field) => Foo,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactic parsing should succeed");
        let (.., e) =
            Gate::<policies::peripheral::ForbidPath, policies::field::RequireBinding>::parse(
                &args, &model,
            );

        assert!(e.iter().any(|diagnostic| {
            matches!(diagnostic.kind(), diagnostic::Kind::FieldMustBeWritable)
        }))
    }

    #[test]
    fn single_field() {
        let peripheral_name = "foo";
        let peripheral_path: Path = parse_quote! { ::external::foo };
        let register_name = "bar";
        let register_ident: Ident = parse_quote! { bar };
        let field_name = "baz";
        let field_ident: Ident = parse_quote! { baz };

        let mut model = Model::new();
        model
            .add_peripheral(Peripheral::new(peripheral_name, 0))
            .add_register(Register::new(register_name, 0))
            .add_write_field(Field::new(field_name, 0, 0));

        let tokens = quote! {
            #peripheral_path::#register_ident::#field_ident(&mut my_field) => Foo,
        };

        let args = syn::parse2::<syntax::Gate>(tokens).expect("syntactic parsing should succeed");
        let (gate, e) =
            Gate::<policies::peripheral::ForbidPath, policies::field::RequireBinding>::parse(
                &args, &model,
            );

        assert!(e.is_empty(), "semantic parsing should succeed");

        let (register, field) = gate
            .get_field(peripheral_name, register_name, field_name)
            .expect("field should exist");

        assert_eq!(register.peripheral_path(), &peripheral_path);
        assert_eq!(register.peripheral().ident, peripheral_name);
        assert_eq!(register.register().ident, register_name);

        assert!(matches!(
            field.entry(),
            policies::field::RequireBinding::Dynamic(..)
        ));
        assert_eq!(field.field().ident, field_name);
    }
}
