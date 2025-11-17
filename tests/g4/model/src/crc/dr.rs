use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        register::Register,
    },
};

pub fn generate() -> Register {
    Register::new(
        "dr",
        0,
        [Field::new(
            "dr",
            0,
            32,
            Access::read_write_asymmetrical(Numericity::Numeric, Numericity::Numeric),
        )],
    )
}
