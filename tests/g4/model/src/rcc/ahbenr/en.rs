use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate(ident: impl AsRef<str>, offset: u8) -> Field {
    Field::new(
        ident,
        offset,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("Disabled", 0),
            Variant::new("Enabled", 1),
        ])),
    )
}
