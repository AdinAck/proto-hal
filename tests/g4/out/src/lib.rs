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
        use crate::{cordic, rcc, read, read_untracked, write, write_from_reset_untracked};

        static mut MOCK_CORDIC: [u32; 3] = [0x0000_0050, 0, 0];

        fn addr_of_cordic() -> usize {
            (&raw const MOCK_CORDIC).addr()
        }

        #[test]
        fn basic() {
            critical_section::with(|cs| {
                let p = unsafe { crate::peripherals() };

                let rcc::ahb1enr::States { cordicen, .. } =
                    rcc::ahb1enr::modify_in_cs(cs, |_, w| {
                        w.cordicen(p.rcc.ahb1enr.cordicen).enabled()
                    });
                let cordic = p.cordic.unmask(cordicen);

                cordic::csr::modify_in_cs(cs, |_, w| {
                    w.func(cordic.csr.func)
                        .sqrt()
                        .scale(cordic.csr.scale)
                        .preserve()
                });

                assert!({
                    let (func, scale) = unsafe {
                        read_untracked! {
                            cordic::csr { func, scale }
                            @base_addr cordic addr_of_cordic()
                        }
                    };

                    func.is_sqrt() && scale.is_n0()
                });

                unsafe {
                    write_from_reset_untracked! {
                        cordic::csr
                        @base_addr cordic addr_of_cordic()
                    }
                };

                assert!({
                    let (func, scale, precision) = unsafe {
                        read_untracked! {
                            cordic::csr { func, scale, precision }
                            @base_addr cordic addr_of_cordic()
                        }
                    };

                    func.is_cos() && scale.is_n0() && precision.is_p20()
                });
            });
        }

        #[test]
        fn wdata() {
            critical_section::with(|cs| {
                let p = unsafe { crate::peripherals() };

                let rcc::ahb1enr::States { cordicen, .. } =
                    rcc::ahb1enr::modify_in_cs(cs, |_, w| {
                        w.cordicen(p.rcc.ahb1enr.cordicen).enabled()
                    });
                let cordic = p.cordic.unmask(cordicen);

                let mut arg = cordic.wdata.arg.unmask(cordic.csr.argsize);

                write! {
                    cordic::wdata {
                        arg: &mut arg => 0xdeadbeef,
                    }
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

                let (r0, r1) = read! {
                    cordic::rdata {
                        res0: &res0,
                        res1: &res1,
                    }
                };

                assert_eq!(r0, 0xbeef);
                assert_eq!(r1, 0xdead);
            });
        }
    }

    mod crc {
        use crate::{crc, rcc, write};

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

                let idr = write! {
                    crc::idr {
                        idr: crc.idr.idr => 0xdeadbeef,
                    }
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
