#![allow(clippy::disallowed_names)]

use proto_hal_model::{Field, Model, Peripheral, Register, Variant, error::Error};

pub fn model() -> Result<Model, Error> {
    let mut model = Model::new();

    let mut foo = model.add_peripheral(Peripheral::new("foo", 0));

    let mut foo0 = foo.add_register(Register::new("foo0", 0).reset(3));

    let mut a = foo0.add_store_field(Field::new("a", 0, 4));

    (0..5).for_each(|i| {
        a.add_variant(Variant::new(format!("V{i}"), i));
    });

    let v5 = a.add_variant(Variant::new("V5", 5)).make_entitlement();

    let mut foo1 = foo.add_register(Register::new("foo1", 4));

    let mut write_requires_v5 = foo1.add_write_field(Field::new("write_requires_v5", 0, 1));

    write_requires_v5.add_variant(Variant::new("Noop", 0));
    write_requires_v5.write_entitlements([[v5]])?;

    let mut bar = model.add_peripheral(Peripheral::new("bar", 0x100));

    bar.add_register(Register::new("bar0", 0));
    bar.add_register(Register::new("bar1", 4));

    Ok(model)
}

#[cfg(test)]
mod tests {
    mod hal {
        use proto_hal_model::{
            diagnostic,
            {Model, peripheral::Peripheral, register::Register},
        };

        /// Create an empty model.
        #[test]
        fn empty() {
            let model = Model::new();

            assert_eq!(model.peripheral_count(), 0);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a model with one peripheral.
        #[test]
        fn one_peripheral() {
            let mut model = Model::new();

            model.add_peripheral(Peripheral::new("foo", 0));

            assert_eq!(model.peripheral_count(), 1);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a model with many disjoint peripherals.
        #[test]
        fn many_peripherals() {
            let mut model = Model::new();

            for (ident, base_addr) in [
                ("foo", 0),
                ("bar", 4),
                ("baz", 8),
                ("dead", 12),
                ("beef", 16),
            ] {
                model.add_peripheral(Peripheral::new(ident, base_addr));
            }

            assert_eq!(model.peripheral_count(), 5);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a model with multiple peripherals with the same identifier.
        ///
        /// Expected behavior: The model will contain one peripheral (the last specified).
        #[test]
        fn peripherals_same_ident() {
            let mut model = Model::new();

            model.add_peripheral(Peripheral::new("foo", 0));
            model.add_peripheral(Peripheral::new("foo", 1));

            assert_eq!(model.peripheral_count(), 1);
            assert_eq!(model.peripherals().last().unwrap().base_addr, 1);
        }

        /// Create a model with multiple peripherals of zero size at the same base address.
        ///
        /// Expected behavior: Since the peripherals are of zero size, they effectively do
        /// not exist and as such there is no error.
        #[test]
        fn zero_size_peripheral_overlap() {
            let mut model = Model::new();

            model.add_peripheral(Peripheral::new("foo", 0));
            model.add_peripheral(Peripheral::new("bar", 0));

            assert_eq!(model.peripheral_count(), 2);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a model with multiple peripherals with overlapping domains.
        ///
        /// Expected behavior: Exactly one diagnostic error is emitted during validation.
        #[test]
        fn peripheral_overlap() {
            let mut model = Model::new();

            let mut foo = model.add_peripheral(Peripheral::new("foo", 0));
            foo.add_register(Register::new("foo0", 0));
            let mut bar = model.add_peripheral(Peripheral::new("bar", 0));
            bar.add_register(Register::new("bar0", 0));

            let mut diagnostics = model.validate().into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.rank(), diagnostic::Rank::Error));
            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Overlap));
            assert!(diagnostics.next().is_none());
        }
    }

    mod peripherals {
        use proto_hal_model::{
            diagnostic,
            {Model, peripheral::Peripheral, register::Register},
        };

        #[test]
        fn many_registers() {
            let mut model = Model::new();

            let mut foo = model.add_peripheral(Peripheral::new("foo", 0));

            for (ident, offset) in [
                ("foo", 0),
                ("bar", 4),
                ("baz", 8),
                ("dead", 12),
                ("beef", 16),
            ] {
                foo.add_register(Register::new(ident, offset));
            }

            assert_eq!(model.register_count(), 5);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        #[test]
        fn register_overlap() {
            let mut model = Model::new();

            let mut foo = model.add_peripheral(Peripheral::new("foo", 0));

            foo.add_register(Register::new("foo", 0));
            foo.add_register(Register::new("bar", 0));

            let mut diagnostics = model.validate().into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.rank(), diagnostic::Rank::Error));
            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Overlap));
            assert!(diagnostics.next().is_none());
        }
    }
}
