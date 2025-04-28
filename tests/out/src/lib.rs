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

            static MOCK_FOO: u32 = foo0::a::Variant::V3 as _;

            #[unsafe(export_name = "__PROTO_HAL_ADDR_OF_FOO")]
            fn addr_of() -> usize {
                (&MOCK_FOO as *const u32).addr()
            }

            #[test]
            fn _unsafe_read() {
                // test harness is properly addressing
                assert_eq!(foo::base_addr(), addr_of());

                assert!(unsafe { foo0::read().a().is_v3() });
            }

            // #[test]
            // fn _unsafe_write() {
            //     unsafe { foo0::write_from_zero(|w| w.a().v4()) };
            //     assert!(unsafe { foo0::read().a().is_v4() });
            // }

            // #[test]
            // fn _unsafe_modify() {
            //     unsafe { foo0::modify(|r, w| w.a().variant(r.a())) }
            // }
        }
    }
}
