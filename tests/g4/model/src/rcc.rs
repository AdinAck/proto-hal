use proto_hal_build::ir::structures::peripheral::Peripheral;

pub mod ahb1enr;

pub fn generate() -> Peripheral {
    Peripheral::new("rcc", 0x4002_1000, [ahb1enr::generate()])
}
