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
                TypeId::of::<crate::foo::foo0::a::Reset>(),
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

            // the unsafe interface tests interact with a shared resource. in order for this to be sound, the tests
            // must run sequentially, which is achieved by requiring acquisition of this lock.
            static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
                let _lock = LOCK.lock().unwrap();
                unsafe { MOCK_FOO = foo0::a::Variant::V1 as _ };
                assert!(unsafe { foo0::read_untracked().a().is_v1() });
            }

            #[test]
            fn unsafe_write() {
                let _lock = LOCK.lock().unwrap();

                unsafe { foo0::write_from_zero_untracked(|w| w.a().v2()) };
                assert!(unsafe { foo0::read_untracked().a().is_v2() });
            }

            #[test]
            fn unsafe_modify() {
                let _lock = LOCK.lock().unwrap();

                unsafe { foo0::write_from_zero_untracked(|w| w.a().v3()) };
                unsafe {
                    foo0::modify_untracked(|r, w| {
                        w.a().variant(foo0::a::Variant::from_bits(r.a() as u32 + 1))
                    })
                };

                assert!(unsafe { foo0::read_untracked().a().is_v4() });
            }
        }
    }

    mod fields {
        use core::any::TypeId;

        use crate::foo::foo0::a;

        #[test]
        fn offset() {
            assert_eq!(a::OFFSET, 0);
        }

        #[test]
        fn reset() {
            assert_eq!(TypeId::of::<a::Reset>(), TypeId::of::<a::V3>());
        }
    }
}
