use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "argsize",
        22,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("Q31", 0).docs(["1 sign bit, 31 fractional bits."]),
            Variant::new("Q15", 1).docs(["1 sign bit, 15 fractional bits."]),
        ])),
    )
    .reset("Q31")
    .docs(["The value format used for arguments."])
}
