use proto_hal_build::ir::structures::register::Register;

pub mod arg;

pub fn generate() -> Register {
    Register::new("wdata", 4, [arg::generate()])
}
