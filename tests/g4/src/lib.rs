#![no_std]

include!(concat!(env!("OUT_DIR"), "/hal.rs"));

#[cfg(test)]
mod tests {
    extern crate std;
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    static mut MOCK_RCC: [u32; 40] = [0; 40];

    #[unsafe(export_name = "__PROTO_HAL_ADDR_OF_RCC")]
    fn addr_of_rcc() -> usize {
        (&raw const MOCK_RCC).addr()
    }

    mod cordic {
        use proto_hal::stasis::Freeze;

        use crate::{cordic, rcc};

        use super::LOCK;
        static mut MOCK_CORDIC: [u32; 3] = [0x0000_0050, 0, 0];

        #[unsafe(export_name = "__PROTO_HAL_ADDR_OF_CORDIC")]
        fn addr_of_cordic() -> usize {
            (&raw const MOCK_CORDIC).addr()
        }

        #[test]
        fn basic() {
            let _lock = LOCK.lock().unwrap();

            let p = unsafe { crate::peripherals() };

            let rcc::ahb1enr::States { cordicen, .. } =
                rcc::ahb1enr::modify(|_, w| w.cordicen(p.rcc.ahb1enr.cordicen).enabled());
            let cordic = p.cordic.unmask(cordicen);

            cordic::csr::modify(|_, w| {
                w.func(cordic.csr.func)
                    .sqrt()
                    .scale(cordic.csr.scale)
                    .preserve()
            });

            assert!({
                let csr = unsafe { cordic::csr::read_untracked() };

                csr.func().is_sqrt() && csr.scale().is_n0()
            });

            unsafe { cordic::csr::write_from_reset_untracked(|w| w) };

            assert!({
                let csr = unsafe { cordic::csr::read_untracked() };

                csr.func().is_cos() && csr.scale().is_n0() && csr.precision().is_p20()
            });
        }

        #[test]
        fn wdata() {
            let _lock = LOCK.lock().unwrap();

            let p = unsafe { crate::peripherals() };

            let rcc::ahb1enr::States { cordicen, .. } =
                rcc::ahb1enr::modify(|_, w| w.cordicen(p.rcc.ahb1enr.cordicen).enabled());
            let cordic = p.cordic.unmask(cordicen);

            let mut arg = cordic.wdata.arg.unmask(cordic.csr.argsize);

            cordic::wdata::write(|w| w.arg(&mut arg, 0xdeadbeefu32));

            assert_eq!(unsafe { MOCK_CORDIC }[1], 0xdeadbeef);
        }

        #[test]
        fn rdata() {
            let _lock = LOCK.lock().unwrap();

            unsafe { MOCK_CORDIC[2] = 0xdeadbeef };

            let p = unsafe { crate::peripherals() };

            let rcc::ahb1enr::States { cordicen, .. } =
                rcc::ahb1enr::modify(|_, w| w.cordicen(p.rcc.ahb1enr.cordicen).enabled());
            let cordic = p.cordic.unmask(cordicen);

            let cordic::csr::States { ressize, .. } =
                cordic::csr::modify(|_, w| w.ressize(cordic.csr.ressize).q15());

            // multiple fields are entitled to these states, so the state must be explicitly frozen.
            let (_, [res0_nres_ent, res1_nres_ent]) = cordic.csr.nres.freeze();
            let (_, [res0_ressize_ent, res1_ressize_ent]) = ressize.freeze();

            let (mut res0, mut res1) = (
                cordic.rdata.res0.unmask(res0_nres_ent, res0_ressize_ent),
                cordic.rdata.res1.unmask(res1_nres_ent, res1_ressize_ent),
            );

            assert_eq!(cordic::rdata::read().res0(&mut res0), 0xbeef);
            assert_eq!(cordic::rdata::read().res1(&mut res1), 0xdead);
        }
    }

    mod crc {
        use crate::{crc, rcc};

        use super::LOCK;
        static mut MOCK_CRC: [u32; 2] = [0, 0];

        #[unsafe(export_name = "__PROTO_HAL_ADDR_OF_CRC")]
        fn addr_of_crc() -> usize {
            (&raw const MOCK_CRC).addr()
        }

        #[test]
        fn basic() {
            let _lock = LOCK.lock().unwrap();

            let p = unsafe { crate::peripherals() };

            let rcc::ahb1enr::States { crcen, .. } =
                rcc::ahb1enr::modify(|_, w| w.crcen(p.rcc.ahb1enr.crcen).enabled());
            let crc = p.crc.unmask(crcen);

            let crc::idr::States { idr } =
                crc::idr::write(|w| w.idr(crc.idr.idr).value::<0xdeadbeef>());

            assert_eq!(idr.value(), unsafe { MOCK_CRC[1] });
        }

        #[test]
        fn inert() {
            let _lock = LOCK.lock().unwrap();

            let p = unsafe { crate::peripherals() };

            let rcc::ahb1enr::States { crcen, .. } =
                rcc::ahb1enr::modify(|_, w| w.crcen(p.rcc.ahb1enr.crcen).enabled());
            let crc = p.crc.unmask(crcen);

            // "rst" need not be specified because it has an inert variant
            crc::cr::write(|w| {
                w.polysize(crc.cr.polysize)
                    .preserve()
                    .rev_in(crc.cr.rev_in)
                    .preserve()
            });
        }
    }
}
