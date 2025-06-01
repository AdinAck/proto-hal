use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "dmaren",
        17,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("Disabled", 0).docs(["No DMA read requests are generated."]),
            Variant::new("Enabled", 1).docs(["Requests are generated on the DMA read channel whenever the [`rrdy`](super::rrdy) flag is set."]),
        ])),
    ).reset("Disabled")
}
