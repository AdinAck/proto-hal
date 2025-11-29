#![no_std]

use macros::scaffolding;

scaffolding!();

#[cfg(test)]
mod tests {
    static mut MOCK_FOO: u32 = u32::MAX;

    fn addr_of_foo() -> usize {
        (&raw const MOCK_FOO).addr()
    }

    mod hal {
        use core::any::{Any, TypeId};

        use crate as hal;

        use hal::foo::foo0::a;

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
        mod unsafe_interface {
            use crate::tests::{MOCK_FOO, addr_of_foo};

            use crate as hal;

            use hal::foo;

            #[test]
            fn unsafe_read() {
                critical_section::with(|_| {
                    unsafe { MOCK_FOO = foo::foo0::a::Variant::V1 as _ };

                    assert!(
                        unsafe {
                            hal::read_untracked! {
                                foo::foo0::a,
                                @base_addr(foo, addr_of_foo())
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
                        hal::write_from_zero_untracked! {
                            foo::foo0::a => V2,
                            @base_addr(foo, addr_of_foo())
                        }
                    };
                    assert!(unsafe {
                        hal::read_untracked! {
                            foo::foo0::a,
                            @base_addr(foo, addr_of_foo())
                        }
                        .is_v2()
                    });
                });
            }

            #[test]
            fn unsafe_modify() {
                critical_section::with(|cs| {
                    unsafe {
                        hal::write_from_zero_untracked! {
                            foo::foo0::a => V3,
                            @base_addr(foo, addr_of_foo())
                        }
                    }

                    unsafe {
                        hal::modify_untracked! {
                            foo::foo0::a => Variant::from_bits(a as u32 + 1),
                            @critical_section(cs),
                            @base_addr(foo, addr_of_foo())
                        }
                    };

                    assert!(unsafe {
                        hal::read_untracked! {
                            foo::foo0::a,
                            @base_addr(foo, addr_of_foo())
                        }
                        .is_v4()
                    });
                });
            }
        }
    }

    mod entitlements {
        use crate::tests::addr_of_foo;

        use crate as hal;

        use hal::foo;

        #[test]
        fn access() {
            let mut p = unsafe { crate::peripherals() };

            let a = p.foo.foo0.a;

            hal::write_in_place! {
                foo::foo0 {
                    a(a) => _,
                },
                @base_addr(foo, addr_of_foo())
            }

            assert!(
                unsafe {
                    hal::read_untracked! {
                        foo::foo0::a,
                        @base_addr(foo, addr_of_foo())
                    }
                }
                .is_v5()
            );

            hal::write! {
                foo {
                    foo1::write_requires_v5(&mut p.foo.foo1.write_requires_v5) => Noop,
                    foo0::a(&a),
                },
                @base_addr(foo, addr_of_foo())
            }
        }
    }
}
