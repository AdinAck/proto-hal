pub mod func;
pub mod precision;
pub mod scale;

use proto_hal_build::ir::structures::register::Register;

pub fn generate() -> Register {
    Register::new(
        "csr",
        0,
        [func::generate(), precision::generate(), scale::generate()],
    )
}
