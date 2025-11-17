use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "rst",
        0,
        1,
        Access::read_write_asymmetrical(
            Numericity::enumerated([Variant::new("Clear", 0), Variant::new("Set", 1)]),
            Numericity::enumerated([Variant::new("Noop", 0).inert(), Variant::new("Set", 1)]),
        ),
    )
}
