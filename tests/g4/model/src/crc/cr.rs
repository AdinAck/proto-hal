pub mod polysize;
pub mod rev_in;
pub mod rev_out;
pub mod rst;

use proto_hal_build::model::structures::register::Register;

pub fn generate() -> Register {
    Register::new(
        "cr",
        8,
        [
            rst::generate(),
            polysize::generate(),
            rev_in::generate(),
            rev_out::generate(),
        ],
    )
    .reset(0)
}
