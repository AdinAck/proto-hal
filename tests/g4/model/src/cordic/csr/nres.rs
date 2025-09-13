use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "nres",
        19,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("One", 0)
                .docs(["One read is needed on the [`rdata`](super::super::rdata) register."]),
            Variant::new("Two", 1)
                .docs(["Two reads are needed on the [`rdata`](super::super::rdata) register."]),
        ])),
    )
}
