use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    let variants = (1..16).map(|i| Variant::new(format!("P{}", i * 4), i));

    Field::new(
        "precision",
        4,
        4,
        Access::read_write(Numericity::enumerated(variants)),
    )
}
