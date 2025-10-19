#![no_std]

use macros::scaffolding;

scaffolding!();

#[cfg(test)]
mod tests {
    mod hal {
        use crate::foo::foo0::a;
        use core::any::{Any, TypeId};

        #[test]
        fn fundamental_peripherals() {
            let p = unsafe { crate::peripherals() };

            assert_eq!(TypeId::of::<a::A<a::V3>>(), p.foo.foo0.a.type_id());
        }
    }

    mod peripherals {
        // nothing yet...
    }

    mod registers {
        use crate::{bar::bar0, foo::foo0};

        #[test]
        fn offset() {
            assert_eq!(foo0::ADDR, 0);
            assert_eq!(bar0::ADDR, 0x100);
        }

        mod unsafe_interface {
            extern crate std;
            use macros::{modify_untracked, read_untracked, write_from_zero_untracked};

            use crate::foo;

            static mut MOCK_FOO: u32 = u32::MAX;

            fn addr_of_foo() -> usize {
                (&raw const MOCK_FOO).addr()
            }

            #[test]
            fn unsafe_read() {
                critical_section::with(|_| {
                    unsafe { MOCK_FOO = foo::foo0::a::Variant::V1 as _ };

                    assert!(
                        unsafe {
                            read_untracked! {
                                foo::foo0 {
                                    a,
                                }
                                @base_addr foo addr_of_foo()
                            }
                        }
                        .is_v1()
                    );
                });
            }

            #[test]
            fn unsafe_write() {
                critical_section::with(|_| {
                    unsafe {
                        write_from_zero_untracked! {
                            foo::foo0 {
                                a => V2,
                            }
                            @base_addr foo addr_of_foo()
                        }
                    };
                    assert!(unsafe {
                        read_untracked! {
                            foo::foo0 {
                                a,
                            }
                            @base_addr foo addr_of_foo()
                        }
                        .is_v2()
                    });
                });
            }

            #[test]
            fn unsafe_modify() {
                critical_section::with(|cs| {
                    unsafe {
                        write_from_zero_untracked! {
                            foo::foo0 {
                                a => V3,
                            }
                            @base_addr foo addr_of_foo()
                        }
                    }

                    unsafe {
                        modify_untracked! {
                            foo::foo0 {
                                a => foo_foo0_a as u32 + 1,
                            }
                            @critical_section cs
                            @base_addr foo addr_of_foo()
                        }
                    };

                    assert!(unsafe {
                        read_untracked! {
                            foo::foo0 {
                                a,
                            }
                            @base_addr foo addr_of_foo()
                        }
                        .is_v4()
                    });
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
        use crate::{foo, write};

        #[test]
        fn access() {
            let mut p = unsafe { crate::peripherals() };

            // let foo::foo0::States { a, .. } = foo::foo0::write(|w| w.a(p.foo.foo0.a).v5());
            let a = write! {
                foo::foo0 {
                    a: p.foo.foo0.a => 5,
                }
            };

            // foo::foo1::write(|w| {
            //     w.write_requires_v5(&mut p.foo.foo1.write_requires_v5, &a)
            //         .noop()
            // });
            write! {
                foo::foo1 {
                    write_requires_v5: &mut p.foo.foo1.write_requires_v5 => Noop,
                }
            }

            // foo::foo1::read().read_requires_v5(&mut p.foo.foo1.read_requires_v5, &a);
        }
    }
}
