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
                rcc::ahb1enr::transition(|reg| reg.cordicen(p.rcc.ahb1enr.cordicen).enabled());
            let cordic = p.cordic.unmask(cordicen);

            cordic::csr::transition(|reg| {
                reg.func(cordic.csr.func)
                    .sqrt()
                    .scale(cordic.csr.scale)
                    .preserve()
            });

            assert!({
                let csr = unsafe { cordic::csr::read_untracked() };

                csr.func().is_sqrt() && csr.scale().is_n0()
            });

            // crate::cordic::wdata::write_from_zero(&cordic.csr.nargs, &cordic.csr.argsize, |w| {
            //     w.arg(0)
            // });
            // crate::cordic::rdata::read(&cordic.csr.nres, &cordic.csr.ressize).res();

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
                rcc::ahb1enr::transition(|reg| reg.cordicen(p.rcc.ahb1enr.cordicen).enabled());
            let cordic = p.cordic.unmask(cordicen);

            cordic::wdata::write(|w| w.arg(&cordic.csr.argsize, 0xdeadbeefu32));

            assert_eq!(unsafe { MOCK_CORDIC }[1], 0xdeadbeef);
        }

        #[test]
        fn rdata() {
            let _lock = LOCK.lock().unwrap();

            unsafe { MOCK_CORDIC[2] = 0xdeadbeef };

            assert_eq!(cordic::rdata::read().res(), 0xdeadbeef);
        }
    }
}
