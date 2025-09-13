use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        register::Register,
    },
};

pub fn generate() -> Register {
    Register::new(
        "idr",
        4,
        [Field::new(
            "idr",
            0,
            32,
            Access::read_write(Numericity::Numeric),
        )],
    )
    .reset(0)
}
