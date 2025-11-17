use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "polysize",
        3,
        2,
        Access::read_write(Numericity::enumerated([
            Variant::new("P32", 0),
            Variant::new("P16", 1),
            Variant::new("P8", 2),
            Variant::new("P7", 3),
        ])),
    )
}
