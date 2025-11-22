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
        use crate::{cordic, rcc};

        use crate as hal;

        static mut MOCK_CORDIC: [u32; 3] = [0x0000_0050, 0, 0];

        fn addr_of_cordic() -> usize {
            (&raw const MOCK_CORDIC).addr()
        }

        #[test]
        fn basic() {
            critical_section::with(|cs| {
                let p = unsafe { crate::peripherals() };

                let cordicen = hal::write! {
                    rcc::ahb1enr::cordicen(p.rcc.ahb1enr.cordicen) => Enabled,
                };

                let cordic = hal::unmask! {
                    rcc::ahb1enr::cordicen(cordicen),
                    cordic(p.cordic),
                };

                cordic::csr::modify_in_cs(cs, |_, w| {
                    w.func(cordic.csr.func)
                        .sqrt()
                        .scale(cordic.csr.scale)
                        .preserve()
                });

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
                let p = unsafe { crate::peripherals() };

                let cordicen = hal::write! {
                    rcc::ahb1enr::cordicen(p.rcc.ahb1enr.cordicen) => Enabled,
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
                }

                assert_eq!(unsafe { MOCK_CORDIC }[1], 0xdeadbeef);
            });
        }

        #[test]
        fn rdata() {
            critical_section::with(|cs| {
                unsafe { MOCK_CORDIC[2] = 0xdeadbeef };

                let p = unsafe { crate::peripherals() };

                let rcc::ahb1enr::States { cordicen, .. } =
                    rcc::ahb1enr::modify_in_cs(cs, |_, w| {
                        w.cordicen(p.rcc.ahb1enr.cordicen).enabled()
                    });
                let cordic = p.cordic.unmask(cordicen);

                let cordic::csr::States { ressize, .. } =
                    cordic::csr::modify_in_cs(cs, |_, w| w.ressize(cordic.csr.ressize).q15());

                // multiple fields are entitled to these states, so the state must be explicitly frozen.
                let (_, [res0_nres_ent, res1_nres_ent]) = cordic.csr.nres.freeze();
                let (_, [res0_ressize_ent, res1_ressize_ent]) = ressize.freeze();

                let (mut res0, mut res1) = (
                    cordic.rdata.res0.unmask(res0_nres_ent, res0_ressize_ent),
                    cordic.rdata.res1.unmask(res1_nres_ent, res1_ressize_ent),
                );

                let rdata = hal::read! {
                    cordic::rdata {
                        res0(&res0),
                        res1(&res1),
                    }
                };

                assert_eq!(rdata.res0, 0xbeef);
                assert_eq!(rdata.res1, 0xdead);
            });
        }
    }

    mod crc {
        use crate::{crc, rcc};

        use crate as hal;

        static mut MOCK_CRC: [u32; 2] = [0, 0];

        fn addr_of_crc() -> usize {
            (&raw const MOCK_CRC).addr()
        }

        #[test]
        fn basic() {
            critical_section::with(|cs| {
                let p = unsafe { crate::peripherals() };

                let rcc::ahb1enr::States { crcen, .. } =
                    rcc::ahb1enr::modify_in_cs(cs, |_, w| w.crcen(p.rcc.ahb1enr.crcen).enabled());
                let crc = p.crc.unmask(crcen);

                let idr = hal::write! {
                    crc::idr::idr(crc.idr.idr) => 0xdeadbeef,
                };

                assert_eq!(0xdeadbeef, unsafe { MOCK_CRC[1] });
            });
        }

        #[test]
        fn inert() {
            critical_section::with(|cs| {
                let p = unsafe { crate::peripherals() };

                let rcc::ahb1enr::States { crcen, .. } =
                    rcc::ahb1enr::modify_in_cs(cs, |_, w| w.crcen(p.rcc.ahb1enr.crcen).enabled());
                let crc = p.crc.unmask(crcen);

                // "rst" need not be specified because it has an inert variant
                // crc::cr::write(|w| {
                //     w.polysize(crc.cr.polysize)
                //         .preserve()
                //         .rev_in(crc.cr.rev_in)
                //         .preserve()
                // });
            });
        }
    }

    mod rcc {
        use core::any::{Any, TypeId};

        use crate::rcc;

        #[test]
        fn reset() {
            let p = unsafe { crate::peripherals() };

            assert_eq!(
                p.rcc.ahb1enr.flashen.type_id(),
                TypeId::of::<rcc::ahb1enr::flashen::Enabled>()
            );
        }
    }
}
