use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "ien",
        16,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("Disabled", 0).docs(["No interrupt requests generated."]),
            Variant::new("Enabled", 1).docs([
                "An interrupt request is generated whenever the [`rrdy`](super::rrdy) flag is set.",
            ]),
        ])),
    )
    .reset("Disabled")
}
