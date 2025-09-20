pub mod en;

use proto_hal_build::ir::structures::register::Register;

#[derive(Clone, Copy)]
pub enum Instance {
    AHB1,
    AHB2,
    APB2,
}

impl Instance {
    fn ident(&self) -> String {
        match self {
            Instance::AHB1 => "ahb1enr",
            Instance::AHB2 => "ahb2enr",
            Instance::APB2 => "apb2enr",
        }
        .to_string()
    }

    fn offset(&self) -> u32 {
        match self {
            Instance::AHB1 => 0x48,
            Instance::AHB2 => 0x4c,
            Instance::APB2 => 0x60,
        }
    }
}

pub fn generate(instance: Instance) -> Register {
    Register::new(
        instance.ident(),
        instance.offset(),
        match instance {
            Instance::AHB1 => vec![
                en::generate("dma1en", 0),
                en::generate("dam2en", 1),
                en::generate("dammux1en", 2),
                en::generate("cordicen", 3),
                en::generate("fmacen", 4),
                en::generate("flashen", 8),
                en::generate("crcen", 12),
            ],
            Instance::AHB2 => vec![
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
            Instance::APB2 => vec![
                en::generate("syscfgen", 0)
                    .docs(["This field enables/disables the peripherals clocks for SYSCFG, COMP, VREFBUF, and OPAMP."]),
                en::generate("tim1en", 11),
                en::generate("spi1en", 12),
                en::generate("tim8en", 13),
                en::generate("usart1en", 14),
                en::generate("spi4en", 15),
                en::generate("tim15en", 16),
                en::generate("tim16en", 17),
                en::generate("tim17en", 18),
                en::generate("tim20en", 20),
                en::generate("sai1en", 21),
                en::generate("hrtim1en", 26),
            ],
        },
    ).reset(match instance {
        Instance::AHB1 => 0x100,
        Instance::AHB2 => 0,
        Instance::APB2 => 0,
    })
}
