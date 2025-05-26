mod csr;

use proto_hal_build::ir::structures::{
    entitlement::Entitlement, peripheral::Peripheral, register::Register,
};

pub fn generate() -> Peripheral {
    let wdata = Register::new("wdata", 4, []);
    let rdata = Register::new("rdata", 8, []);

    let cordic = Peripheral::new("cordic", 0x4002_0c00, [csr::generate(), wdata, rdata])
        .entitlements([Entitlement::to("rcc::ahb1enr::cordicen::Enabled")]);

    cordic
}
