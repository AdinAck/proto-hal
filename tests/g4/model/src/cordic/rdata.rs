use proto_hal_build::model::structures::register::Register;

pub mod res;
pub mod res0;
pub mod res1;

pub fn generate() -> Register {
    Register::new(
        "rdata",
        8,
        [res::generate(), res0::generate(), res1::generate()],
    )
}
