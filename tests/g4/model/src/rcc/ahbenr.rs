pub mod en;

use proto_hal_build::ir::structures::register::Register;

#[derive(Clone, Copy)]
pub enum Instance {
    I1,
    I2,
}

impl Instance {
    fn ident(&self) -> String {
        match self {
            Instance::I1 => "ahb1enr",
            Instance::I2 => "ahb2enr",
        }
        .to_string()
    }

    fn offset(&self) -> u32 {
        match self {
            Instance::I1 => 0x48,
            Instance::I2 => 0x4c,
        }
    }
}

pub fn generate(instance: Instance) -> Register {
    Register::new(
        instance.ident(),
        instance.offset(),
        match instance {
            Instance::I1 => vec![
                en::generate("dma1en", 0),
                en::generate("dam2en", 1),
                en::generate("dammux1en", 2),
                en::generate("cordicen", 3),
                en::generate("fmacen", 4),
                en::generate("flashen", 8),
                en::generate("crcen", 12),
            ],
            Instance::I2 => vec![
                en::generate("gpioaen", 0),
                en::generate("gpioben", 1),
                en::generate("gpiocen", 2),
                en::generate("gpioden", 3),
                en::generate("gpioeen", 4),
                en::generate("gpiofen", 5),
                en::generate("gpiogen", 6),
                en::generate("adc12en", 13),
                en::generate("adc345en", 14),
                en::generate("dac1en", 16),
                en::generate("dac2en", 17),
                en::generate("dac3en", 18),
                en::generate("dac4en", 19),
                en::generate("aesen", 24),
                en::generate("rngen", 26),
            ],
        },
    )
    .reset(match instance {
        Instance::I1 => 0x100,
        Instance::I2 => 0,
    })
}
