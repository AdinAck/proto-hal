use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "rrdy",
        31,
        1,
        Access::read(Numericity::enumerated([
            Variant::new("NoData", 0).docs(["No new data in output register."]),
            Variant::new("Ready", 1).docs(["[`rdata`](super::super::rdata) contains a result."]),
        ])),
    )
}
