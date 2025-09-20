pub mod enr;

use proto_hal_build::ir::structures::peripheral::Peripheral;

pub fn generate() -> Peripheral {
    Peripheral::new(
        "rcc",
        0x4002_1000,
        [
            enr::generate(enr::Instance::AHB1),
            enr::generate(enr::Instance::AHB2),
            enr::generate(enr::Instance::APB2),
        ],
    )
}
