use proto_hal_build::ir::{
    access::Access,
    structures::{
        field::{Field, Numericity},
        register::Register,
        variant::Variant,
    },
};

#[derive(Clone, Copy)]
pub enum Instance {
    I1,
    I2,
    I3,
    I4,
}

impl Instance {
    fn ident(&self) -> String {
        match self {
            Instance::I1 => "exticr1",
            Instance::I2 => "exticr2",
            Instance::I3 => "exticr3",
            Instance::I4 => "exticr4",
        }
        .to_string()
    }

    fn offset(&self) -> u32 {
        match self {
            Instance::I1 => 0x08,
            Instance::I2 => 0x0c,
            Instance::I3 => 0x10,
            Instance::I4 => 0x14,
        }
    }
}

pub fn generate(instance: Instance) -> Register {
    Register::new(
        instance.ident(),
        instance.offset(),
        [Field::new(
            "extix",
            0,
            4,
            Access::read_write(Numericity::enumerated([
                Variant::new("PA", 0),
                Variant::new("PB", 1),
                Variant::new("PC", 2),
                Variant::new("PD", 3),
                Variant::new("PE", 4),
                Variant::new("PF", 5),
                Variant::new("PG", 6),
            ])),
        )
        .array(4, |i| format!("exti{i}"))],
    )
    .reset(0)
}
