use proto_hal_build::model::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "dmawen",
        18,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("Disabled", 0).docs(["No DMA write requests are generated."]),
            Variant::new("Enabled", 1).docs([
                "Requests are generated on the DMA write channel whenever no operation is pending.",
            ]),
        ])),
    )
}
