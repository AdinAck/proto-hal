use proto_hal_build::ir::structures::register::Register;

pub mod cordicen;

pub fn generate() -> Register {
    Register::new("ahb1enr", 0x48, [cordicen::generate()])
}
