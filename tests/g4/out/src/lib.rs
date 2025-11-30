#![no_std]

use macros::scaffolding;

scaffolding!();

#[cfg(test)]
mod tests {
    extern crate std;

    static mut MOCK_RCC: [u32; 40] = [0; 40];

    fn addr_of_rcc() -> usize {
        (&raw const MOCK_RCC).addr()
    }

    mod cordic {
        use super::addr_of_rcc;

        use crate as hal;

        use hal::{cordic, rcc};

        static mut MOCK_CORDIC: [u32; 3] = [0x0000_0050, 0, 0];

        fn addr_of_cordic() -> usize {
            (&raw const MOCK_CORDIC).addr()
        }

        #[test]
        fn basic() {
            critical_section::with(|cs| {
                let p = unsafe { hal::peripherals() };

                let cordicen = hal::modify! {
                    @critical_section(cs),
                    rcc::ahb1enr::cordicen(p.rcc.ahb1enr.cordicen) => Enabled,
                    @base_addr(rcc, addr_of_rcc())
                };

                let cordic = hal::unmask! {
                    rcc::ahb1enr::cordicen(cordicen),
                    cordic(p.cordic),
                };

                hal::modify! {
                    @critical_section(cs),
                    cordic::csr {
                        func(cordic.csr.func) => Sqrt,
                        scale(&cordic.csr.scale),
                    },
                    @base_addr(cordic, addr_of_cordic()),
                };

                assert!({
                    let csr = unsafe {
                        hal::read_untracked! {
                            cordic::csr { func, scale },
                            @base_addr(cordic, addr_of_cordic())
                        }
                    };

                    csr.func.is_sqrt() && csr.scale.is_n0()
                });

                unsafe {
                    hal::write_from_reset_untracked! {
                        cordic::csr,
                        @base_addr(cordic, addr_of_cordic())
                    }
                };

                assert!({
                    let csr = unsafe {
                        hal::read_untracked! {
                            cordic::csr { func, scale, precision },
                            @base_addr(cordic, addr_of_cordic())
                        }
                    };

                    csr.func.is_cos() && csr.scale.is_n0() && csr.precision.is_p20()
                });
            });
        }

        #[test]
        fn wdata() {
            critical_section::with(|cs| {
                let p = unsafe { hal::peripherals() };

                let cordicen = hal::modify! {
                    @critical_section(cs),
                    rcc::ahb1enr::cordicen(p.rcc.ahb1enr.cordicen) => Enabled,
                    @base_addr(rcc, addr_of_rcc())
                };

                let cordic = hal::unmask! {
                    cordic(p.cordic),
                    rcc::ahb1enr::cordicen(cordicen)
                };

                let mut arg = hal::unmask! {
                    cordic::csr::argsize(cordic.csr.argsize),
                    cordic::wdata::arg(cordic.wdata.arg),
                };

                hal::write! {
                    cordic::wdata::arg(&mut arg) => 0xdeadbeef,
                    @base_addr(cordic, addr_of_cordic()),
                }

                assert_eq!(unsafe { MOCK_CORDIC }[1], 0xdeadbeef);
            });
        }

        #[test]
        fn rdata() {
            critical_section::with(|cs| {
                unsafe { MOCK_CORDIC[2] = 0xdeadbeef };

                let p = unsafe { hal::peripherals() };

                let cordicen = hal::modify! {
                    @critical_section(cs),
                    rcc::ahb1enr::cordicen(p.rcc.ahb1enr.cordicen) => Enabled,
                    @base_addr(rcc, addr_of_rcc())
                };

                let cordic = hal::unmask! {
                    cordic(p.cordic),
                    rcc::ahb1enr::cordicen(cordicen)
                };

                let ressize = hal::modify! {
                    @critical_section(cs),
                    cordic::csr::ressize(cordic.csr.ressize) => Q15,
                    @base_addr(cordic, addr_of_cordic()),
                };

                let (mut res0, mut res1) = hal::unmask! {
                    cordic::rdata::res0(cordic.rdata.res0),
                    cordic::rdata::res1(cordic.rdata.res1),
                    cordic::csr::ressize(ressize),
                    cordic::csr::nres(cordic.csr.nres),
                };

                let rdata = hal::read! {
                    cordic::rdata {
                        res0(&mut res0),
                        res1(&mut res1),
                    },
                    @base_addr(cordic, addr_of_cordic()),
                };

                assert_eq!(rdata.res0, 0xbeef);
                assert_eq!(rdata.res1, 0xdead);
            });
        }
    }

    mod crc {
        use core::any::{Any, TypeId};

        use super::addr_of_rcc;

        use crate as hal;

        use hal::{crc, rcc};

        static mut MOCK_CRC: [u32; 2] = [0, 0];

        fn addr_of_crc() -> usize {
            (&raw const MOCK_CRC).addr()
        }

        #[test]
        fn basic() {
            critical_section::with(|cs| {
                let p = unsafe { hal::peripherals() };

                let crcen = hal::modify! {
                    @critical_section(cs),
                    rcc::ahb1enr::crcen(p.rcc.ahb1enr.crcen) => Enabled,
                    @base_addr(rcc, addr_of_rcc())
                };

                let crc = hal::unmask! {
                    crc(p.crc),
                    rcc::ahb1enr::crcen(crcen),
                };

                let idr = hal::write! {
                    crc::idr::idr(crc.idr.idr) => 0xdeadbeef,
                    @base_addr(crc, addr_of_crc()),
                };

                assert_eq!(0xdeadbeef, unsafe { MOCK_CRC[1] });
                assert_eq!(
                    idr.type_id(),
                    TypeId::of::<crc::idr::idr::Idr<proto_hal::stasis::UInt32<0xdeadbeef>>>()
                );
            });
        }

        #[test]
        fn inert() {
            critical_section::with(|cs| {
                let p = unsafe { hal::peripherals() };

                let crcen = hal::modify! {
                    @critical_section(cs),
                    rcc::ahb1enr::crcen(p.rcc.ahb1enr.crcen) => Enabled,
                    @base_addr(rcc, addr_of_rcc())
                };

                let crc = hal::unmask! {
                    crc(p.crc),
                    rcc::ahb1enr::crcen(crcen),
                };

                // "rst" need not be specified because it has an inert variant
                hal::write! {
                    crc::cr {
                        polysize(crc.cr.polysize) => P32,
                        rev_in(crc.cr.rev_in) => NoEffect,
                        rev_out(crc.cr.rev_out) => NoEffect,
                    },
                    @base_addr(crc, addr_of_crc())
                };
            });
        }
    }

    mod rcc {
        use core::any::{Any, TypeId};

        use crate as hal;

        use hal::rcc;

        #[test]
        fn reset() {
            let p = unsafe { hal::peripherals() };

            assert_eq!(
                p.rcc.ahb1enr.flashen.type_id(),
                TypeId::of::<rcc::ahb1enr::flashen::Flashen<rcc::ahb1enr::flashen::Enabled>>()
            );
        }
    }
}
