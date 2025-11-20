use proto_hal_build::model::structures::{
    field::Field, model::PeripheralEntry, register::Register, variant::Variant,
};

pub fn ahb2enr<'cx>(rcc: &mut PeripheralEntry<'cx>) {
    let mut ahb2enr = rcc.add_register(Register::new("ahb2enr", 0x4c).reset(0));

    for (ident, offset) in [
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
    ] {
        let mut en = ahb2enr.add_store_field(Field::new(ident, offset, 1));

        en.add_variant(Variant::new("Disabled", 0));
        en.add_variant(Variant::new("Enabled", 1));
    }
}
