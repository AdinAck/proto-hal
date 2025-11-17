pub mod cr;
pub mod dr;
pub mod idr;

use proto_hal_build::model::structures::{entitlement::Entitlement, peripheral::Peripheral};

pub fn generate() -> Peripheral {
    Peripheral::new(
        "crc",
        0x4002_3000,
        [dr::generate(), idr::generate(), cr::generate()],
    )
    .entitlements([Entitlement::to("rcc::ahb1enr::crcen::Enabled")])
}
