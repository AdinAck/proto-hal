use proto_hal_build::model::{
    Entitlement, Field, Register, Variant,
    model::{PeripheralEntry, RegisterEntry},
};

pub struct Output {
    pub cordicen: Entitlement,
    pub crcen: Entitlement,
}

pub fn ahb1enr<'cx>(rcc: &mut PeripheralEntry<'cx>) -> Output {
    let mut ahb1enr = rcc.add_register(Register::new("ahb1enr", 0x48).reset(0x100));

    add_field(&mut ahb1enr, "dma1en", 0);
    add_field(&mut ahb1enr, "dma2en", 1);
    add_field(&mut ahb1enr, "dmamux1en", 2);
    let cordicen = add_field(&mut ahb1enr, "cordicen", 3);
    add_field(&mut ahb1enr, "fmacen", 4);
    add_field(&mut ahb1enr, "flashen", 8);
    let crcen = add_field(&mut ahb1enr, "crcen", 12);

    Output { cordicen, crcen }
}

fn add_field<'cx>(
    ahb1enr: &mut RegisterEntry<'cx>,
    ident: impl AsRef<str>,
    offset: u8,
) -> Entitlement {
    let mut en = ahb1enr.add_store_field(Field::new(ident, offset, 1));

    en.add_variant(Variant::new("Disabled", 0));
    en.add_variant(Variant::new("Enabled", 1))
        .make_entitlement()
}
