use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    let variants = (0..8).map(|i| Variant::new(format!("N{i}"), i));

    Field::new(
        "scale",
        8,
        3,
        Access::read_write(Numericity::enumerated(variants)),
    )
}
