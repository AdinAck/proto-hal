use proto_hal_build::model::structures::{field::Field, model::RegisterEntry, variant::Variant};

pub fn en<'cx>(enr: &mut RegisterEntry<'cx>, ident: impl AsRef<str>, offset: u8) {
    let mut en = enr.add_store_field(Field::new(ident, offset, 1));

    en.add_variant(Variant::new("Disabled", 0));
    en.add_variant(Variant::new("Enabled", 1));
}
