use proto_hal_build::ir::structures::register::Register;

pub mod res;

pub fn generate() -> Register {
    Register::new("rdata", 8, [res::generate()])
}
