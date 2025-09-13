use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    Field::new(
        "nargs",
        20,
        1,
        Access::read_write(Numericity::enumerated([
            Variant::new("One", 0)
                .docs(["One write is needed to the [`wdata`](super::super::wdata) register."]),
            Variant::new("Two", 1)
                .docs(["Two writes are needed to the [`wdata`](super::super::wdata) register."]),
        ])),
    )
}
