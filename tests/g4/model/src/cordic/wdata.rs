use proto_hal_build::ir::structures::register::Register;

pub mod arg;
pub mod arg0;
pub mod arg1;

pub fn generate() -> Register {
    Register::new(
        "wdata",
        4,
        [arg::generate(), arg0::generate(), arg1::generate()],
    )
}
