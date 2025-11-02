use proto_hal_build::ir::{
    access::{Access, AccessProperties},
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
        hal::Hal,
        peripheral::Peripheral,
        register::Register,
        variant::Variant,
    },
};

pub fn generate() -> Hal {
    Hal::new([
        Peripheral::new(
            "foo",
            0,
            [
                Register::new(
                    "foo0",
                    0,
                    [Field::new(
                        "a",
                        0,
                        4,
                        Access::read_write(Numericity::enumerated(
                            (0..6).map(|i| Variant::new(format!("V{i}"), i)),
                        )),
                    )],
                )
                .reset(3),
                Register::new(
                    "foo1",
                    4,
                    [
                        Field::new(
                            "write_requires_v5",
                            0,
                            1,
                            Access::Write(
                                AccessProperties::enumerated([Variant::new("Noop", 0)])
                                    .entitlements([Entitlement::to("foo::foo0::a::V5")]),
                            ),
                        ),
                        Field::new(
                            "read_requires_v5",
                            1,
                            1,
                            Access::Read(
                                AccessProperties::numeric()
                                    .entitlements([Entitlement::to("foo::foo0::a::V5")]),
                            ),
                        ),
                    ],
                ),
            ],
        ),
        Peripheral::new(
            "bar",
            0x100,
            [Register::new("bar0", 0, []), Register::new("bar1", 4, [])],
        ),
    ])
}

#[cfg(test)]
mod tests {
    mod hal {
        use proto_hal_build::ir::{
            diagnostic,
            structures::{hal::Hal, peripheral::Peripheral, register::Register},
        };

        /// Create an empty HAL.
        #[test]
        fn empty() {
            let hal = Hal::new([]);

            assert!(hal.peripherals.is_empty());

            let diagnostics = hal.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a HAL with one peripheral.
        #[test]
        fn one_peripheral() {
            let hal = Hal::new([Peripheral::new("foo", 0, [])]);

            assert_eq!(hal.peripherals.len(), 1);

            let diagnostics = hal.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a HAL with many disjoint peripherals.
        #[test]
        fn many_peripherals() {
            let hal = Hal::new([
                Peripheral::new("foo", 0, []),
                Peripheral::new("bar", 4, []),
                Peripheral::new("baz", 8, []),
                Peripheral::new("dead", 12, []),
                Peripheral::new("beef", 16, []),
            ]);

            assert_eq!(hal.peripherals.len(), 5);

            let diagnostics = hal.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a HAL with multiple peripherals with the same identifier.
        ///
        /// Expected behavior: The HAL will contain one peripheral (the last specified).
        #[test]
        fn peripherals_same_ident() {
            let hal = Hal::new([Peripheral::new("foo", 0, []), Peripheral::new("foo", 1, [])]);

            assert_eq!(hal.peripherals.len(), 1);
            assert_eq!(hal.peripherals.values().last().unwrap().base_addr, 1);
        }

        /// Create a HAL with multiple peripherals of zero size at the same base address.
        ///
        /// Expected behavior: Since the peripherals are of zero size, they effectively do
        /// not exist and as such there is no error.
        #[test]
        fn zero_size_peripheral_overlap() {
            let hal = Hal::new([Peripheral::new("foo", 0, []), Peripheral::new("bar", 0, [])]);

            assert_eq!(hal.peripherals.len(), 2);

            let diagnostics = hal.validate();

            assert!(diagnostics.is_empty());
        }

        /// Create a HAL with multiple peripherals with overlapping domains.
        ///
        /// Expected behavior: Exactly one diagnostic error is emitted during validation.
        #[test]
        fn peripheral_overlap() {
            let hal = Hal::new([
                Peripheral::new("foo", 0, [Register::new("foo0", 0, [])]),
                Peripheral::new("bar", 0, [Register::new("bar0", 0, [])]),
            ]);

            let mut diagnostics = hal.validate().into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Error));
            // TODO: match exact diagnostic kind
            assert!(diagnostics.next().is_none());
        }
    }

    mod peripherals {
        use proto_hal_build::ir::{
            diagnostic::{self, Context},
            structures::{peripheral::Peripheral, register::Register},
        };

        #[test]
        fn empty() {
            let peripheral = Peripheral::new("foo", 0, []);

            assert!(peripheral.registers.is_empty());

            let diagnostics = peripheral.validate(&Context::new());

            assert!(diagnostics.is_empty());
        }

        #[test]
        fn one_register() {
            let peripheral = Peripheral::new("foo", 0, [Register::new("foo0", 0, [])]);

            assert_eq!(peripheral.registers.len(), 1);

            let diagnostics = peripheral.validate(&Context::new());

            assert!(diagnostics.is_empty());
        }

        #[test]
        fn many_registers() {
            let peripheral = Peripheral::new(
                "foo",
                0,
                [
                    Register::new("foo", 0, []),
                    Register::new("bar", 4, []),
                    Register::new("baz", 8, []),
                    Register::new("dead", 12, []),
                    Register::new("beef", 16, []),
                ],
            );

            assert_eq!(peripheral.registers.len(), 5);

            let diagnostics = peripheral.validate(&Context::new());

            assert!(diagnostics.is_empty());
        }

        #[test]
        fn register_overlap() {
            let peripheral = Peripheral::new(
                "foo",
                0,
                [Register::new("foo", 0, []), Register::new("bar", 0, [])],
            );

            let mut diagnostics = peripheral.validate(&Context::new()).into_iter();

            let diagnostic = diagnostics.next().unwrap();

            assert!(matches!(diagnostic.kind(), diagnostic::Kind::Error));
            // TODO: match exact diagnostic kind
            assert!(diagnostics.next().is_none());
        }
    }
}
