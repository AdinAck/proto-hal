pub mod argsize;
pub mod dmaren;
pub mod dmawen;
pub mod func;
pub mod ien;
pub mod nargs;
pub mod nres;
pub mod precision;
pub mod ressize;
pub mod rrdy;
pub mod scale;

use proto_hal_build::model::structures::register::Register;

pub fn generate() -> Register {
    Register::new(
        "csr",
        0,
        [
            func::generate(),
            precision::generate(),
            scale::generate(),
            ien::generate(),
            dmaren::generate(),
            dmawen::generate(),
            nres::generate(),
            nargs::generate(),
            ressize::generate(),
            argsize::generate(),
            rrdy::generate(),
        ],
    )
    .reset(0x50)
}
