pub mod en;

use proto_hal_build::model::structures::{model::PeripheralEntry, register::Register};

use en::en;

#[derive(Clone, Copy)]
pub enum Instance {
    AHB1,
    AHB2,
}

impl Instance {
    const AHB1_FIELDS: &[(&str, u8)] = &[
        ("dma1en", 0),
        ("dam2en", 1),
        ("dammux1en", 2),
        ("cordicen", 3),
        ("fmacen", 4),
        ("flashen", 8),
        ("crcen", 12),
    ];

    const AHB2_FIELDS: &[(&str, u8)] = &[
        ("gpioaen", 0),
        ("gpioben", 1),
        ("gpiocen", 2),
        ("gpioden", 3),
        ("gpioeen", 4),
        ("gpiofen", 5),
        ("gpiogen", 6),
        ("adc12en", 13),
        ("adc345en", 14),
        ("dac1en", 16),
        ("dac2en", 17),
        ("dac3en", 18),
        ("dac4en", 19),
        ("aesen", 24),
        ("rngen", 26),
    ];

    fn ident(&self) -> String {
        match self {
            Self::AHB1 => "ahb1enr",
            Self::AHB2 => "ahb2enr",
        }
        .to_string()
    }

    fn offset(&self) -> u32 {
        match self {
            Self::AHB1 => 0x48,
            Self::AHB2 => 0x4c,
        }
    }

    fn reset(&self) -> u32 {
        match self {
            Self::AHB1 => 0x100,
            Self::AHB2 => 0,
        }
    }

    fn fields(&self) -> impl Iterator<Item = (&str, u8)> {
        match self {
            Self::AHB2 => &Self::AHB1_FIELDS,
            Self::AHB1 => &Self::AHB2_FIELDS,
        }
        .into_iter()
        .copied()
    }
}

pub fn enr<'cx>(rcc: &mut PeripheralEntry<'cx>, instance: Instance) {
    let mut enr = rcc
        .add_register(Register::new(instance.ident(), instance.offset()).reset(instance.reset()));

    for (ident, offset) in instance.fields() {
        en(&mut enr, ident, offset);
    }
}
