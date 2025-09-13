#![no_std]

include!(concat!(env!("OUT_DIR"), "/hal.rs"));

#[cfg(test)]
mod tests {
    mod hal {
        use core::any::{Any, TypeId};

        #[test]
        fn fundamental_peripherals() {
            let p = unsafe { crate::peripherals() };

            assert_eq!(
                TypeId::of::<crate::foo::foo0::a::V3>(),
                p.foo.foo0.a.type_id(),
            );
        }
    }

    mod peripherals {
        // nothing yet...
    }

    mod registers {
        use crate::{bar::bar0, foo::foo0};

        #[test]
        fn offset() {
            assert_eq!(foo0::OFFSET, 0);
            assert_eq!(bar0::OFFSET, 0);
        }

        mod unsafe_interface {
            extern crate std;
            use crate::foo::{self, foo0};

            static mut MOCK_FOO: u32 = u32::MAX;

            #[unsafe(export_name = "__PROTO_HAL_ADDR_OF_FOO")]
            fn addr_of() -> usize {
                (&raw const MOCK_FOO).addr()
            }

            #[test]
            fn harness_addressing() {
                assert_eq!(foo::base_addr(), addr_of());
            }

            #[test]
            fn unsafe_read() {
                critical_section::with(|_| {
                    unsafe { MOCK_FOO = foo0::a::Variant::V1 as _ };
                    assert!(unsafe { foo0::read_untracked().a().is_v1() });
                });
            }

            #[test]
            fn unsafe_write() {
                critical_section::with(|_| {
                    unsafe { foo0::write_from_zero_untracked(|w| w.a(foo0::a::WriteVariant::V2)) };
                    assert!(unsafe { foo0::read_untracked().a().is_v2() });
                });
            }

            #[test]
            fn unsafe_modify() {
                critical_section::with(|cs| {
                    unsafe { foo0::write_from_zero_untracked(|w| w.a(foo0::a::WriteVariant::V3)) };
                    unsafe {
                        foo0::modify_untracked(cs, |r, w| {
                            w.a(foo0::a::Variant::from_bits(r.a() as u32 + 1))
                        })
                    };

                    assert!(unsafe { foo0::read_untracked().a().is_v4() });
                });
            }
        }
    }

    mod fields {
        use crate::foo::foo0::a;

        #[test]
        fn offset() {
            assert_eq!(a::OFFSET, 0);
        }
    }

    mod entitlements {
        use crate::foo;

        #[test]
        fn access() {
            let mut p = unsafe { crate::peripherals() };

            let foo::foo0::States { a, .. } = foo::foo0::write(|w| w.a(p.foo.foo0.a).v5());

            foo::foo1::write(|w| {
                w.write_requires_v5(&mut p.foo.foo1.write_requires_v5, &a)
                    .noop()
            });

            foo::foo1::read().read_requires_v5(&mut p.foo.foo1.read_requires_v5, &a);
        }
    }
}
