#![allow(clippy::disallowed_names)]

use phm::Composition;

/// The device's model description source.
pub const DEVICE: &str = include_str!("device.phm");

/// The device model, evaluated from [`DEVICE`].
///
/// Both the HAL codegen (`out/build.rs`) and the gate macros consume this
/// model description, so the `.phm` source is the single point of truth for
/// the test device.
///
/// Diagnostics are *not* reported here — the drivers report them naturally:
/// `out/build.rs` fails the build with full reports, and this crate's `main`
/// renders them for terminal use.
pub fn compose() -> Composition {
    let (file, ..) = syntax::parse(DEVICE, 0);

    let Some(file) = file else {
        return Composition::new();
    };

    let units = vec![proto_hal_build::model::elaborate::Unit {
        file,
        imports: Default::default(),
    }];

    let (composition, ..) = proto_hal_build::model::elaborate::elaborate(&units);

    composition
}

#[cfg(test)]
mod tests {
    mod hal {
        use phm::{
            Composition, diagnostic, peripheral::Peripheral, prelude::*, register::Register,
        };

        /// Create an empty model.
        #[test]
        fn empty() {
            let model = Composition::new();

            assert_eq!(model.peripheral_count(), 0);

            let diagnostics = model.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a model with one peripheral.
        #[test]
        fn one_peripheral() {
            let mut model = Composition::new();

            model.add_peripheral(Peripheral::new("foo", 0));

            assert_eq!(model.peripheral_count(), 1);

            let (.., diagnostics) = model.finish();
            assert!(diagnostics.is_empty());
        }

        /// Create a model with many disjoint peripherals.
        #[test]
        fn many_peripherals() {
            let mut model = Composition::new();

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

            let (.., diagnostics) = model.finish();
            assert!(diagnostics.is_empty());
        }

        /// Create a model with multiple peripherals with the same identifier.
        ///
        /// Expected behavior: The model will contain one peripheral (the last specified) and validation will fail.
        #[test]
        fn peripherals_same_ident() {
            let mut model = Composition::new();

            model.add_peripheral(Peripheral::new("foo", 0));
            model.add_peripheral(Peripheral::new("foo", 1));

            assert_eq!(model.peripheral_count(), 1);
            assert_eq!(model.peripherals().last().unwrap().base_addr, 1);

            let (.., diagnostics) = model.finish();

            let mut diagnostics = diagnostics.into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.rank(), diagnostic::Rank::Warning));
            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Exists));

            // bonus diagnostic
            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.rank(), diagnostic::Rank::Error));
            assert!(matches!(
                diagnostic.kind(),
                diagnostic::Kind::AddressUnaligned
            ));
            assert!(diagnostics.next().is_none());
        }

        /// Create a model with multiple peripherals of zero size at the same base address.
        ///
        /// Expected behavior: Since the peripherals are of zero size, they effectively do
        /// not exist and as such there is no error.
        #[test]
        fn zero_size_peripheral_overlap() {
            let mut model = Composition::new();

            model.add_peripheral(Peripheral::new("foo", 0));
            model.add_peripheral(Peripheral::new("bar", 0));

            assert_eq!(model.peripheral_count(), 2);

            let (.., diagnostics) = model.finish();
            assert!(diagnostics.is_empty());
        }

        /// Create a model with multiple peripherals with overlapping domains.
        ///
        /// Expected behavior: Exactly one diagnostic error is emitted during validation.
        #[test]
        fn peripheral_overlap() {
            let mut model = Composition::new();

            let mut foo = model.add_peripheral(Peripheral::new("foo", 0));
            foo.add_register(Register::new("foo0", 0));
            let mut bar = model.add_peripheral(Peripheral::new("bar", 0));
            bar.add_register(Register::new("bar0", 0));

            let (.., diagnostics) = model.finish();

            let mut diagnostics = diagnostics.into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.rank(), diagnostic::Rank::Error));
            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Overlap));
            assert!(diagnostics.next().is_none());
        }
    }

    mod peripherals {
        use phm::{
            Composition, diagnostic, peripheral::Peripheral, prelude::*, register::Register,
        };

        #[test]
        fn many_registers() {
            let mut model = Composition::new();

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

            let (.., diagnostics) = model.finish();
            assert!(diagnostics.is_empty());
        }

        #[test]
        fn register_overlap() {
            let mut model = Composition::new();

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
