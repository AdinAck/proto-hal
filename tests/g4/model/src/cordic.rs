mod csr;
mod rdata;
mod wdata;

use proto_hal_build::ir::structures::{entitlement::Entitlement, peripheral::Peripheral};

pub fn generate() -> Peripheral {
    let cordic = Peripheral::new(
        "cordic",
        0x4002_0c00,
        [csr::generate(), wdata::generate(), rdata::generate()],
    )
    .entitlements([Entitlement::to("rcc::ahb1enr::cordicen::Enabled")]);

    cordic
}
