use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "rev_in",
        5,
        2,
        Access::read_write(Numericity::enumerated([
            Variant::new("NoEffect", 0),
            Variant::new("Byte", 1),
            Variant::new("HalfWord", 2),
            Variant::new("Word", 3),
        ])),
    )
}
