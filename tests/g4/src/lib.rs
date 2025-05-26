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
                    .n1()
            });

            assert!({
                let csr = unsafe { cordic::csr::read() };

                csr.func().is_sqrt() && csr.scale().is_n1()
            })

            // crate::cordic::wdata::write_from_zero(&cordic.csr.nargs, &cordic.csr.argsize, |w| {
            //     w.arg(0)
            // });
            // crate::cordic::rdata::read(&cordic.csr.nres, &cordic.csr.ressize).res();
        }
    }
}
