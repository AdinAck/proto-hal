use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "reset",
        0,
        1,
        Access::read_write_asymmetrical(
            Numericity::enumerated([Variant::new("Clear", 0), Variant::new("Set", 1)]),
            Numericity::enumerated([Variant::new("Set", 1)]),
        ),
    )
}
