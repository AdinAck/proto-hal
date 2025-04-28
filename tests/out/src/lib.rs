#![no_std]

include!(concat!(env!("OUT_DIR"), "/hal.rs"));

#[cfg(test)]
mod tests {
    mod peripherals {
        use crate::{bar, foo};

        #[test]
        fn base_addr() {
            assert_eq!(foo::BASE_ADDR, 0);
            assert_eq!(bar::BASE_ADDR, 0x100);
        }
    }

    mod registers {
        extern crate std;
        use crate::{bar::bar0, foo::foo0};

        #[test]
        fn offset() {
            assert_eq!(foo0::OFFSET, 0);
            assert_eq!(bar0::OFFSET, 0);
        }

        mod unsafe_interface {
            extern crate std;

            use crate::foo::{self, foo0};

            static mut MOCK_FOO: u32 = 0;

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
                unsafe { MOCK_FOO = foo0::a::Variant::V1 as _ };
                assert!(unsafe { foo0::read().a().is_v1() });
            }

            #[test]
            fn unsafe_write() {
                unsafe { foo0::write_from_zero(|w| w.a().v2()) };
                assert!(unsafe { foo0::read().a().is_v2() });
            }

            #[test]
            fn unsafe_modify() {
                unsafe { foo0::write_from_zero(|w| w.a().v3()) };
                unsafe {
                    foo0::modify(|r, w| {
                        w.a().variant(foo0::a::Variant::from_bits(r.a() as u32 + 1))
                    })
                }
                assert!(unsafe { foo0::read().a().is_v4() });
            }
        }
    }
}
