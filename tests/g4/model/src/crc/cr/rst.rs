use proto_hal_build::model::structures::{field::Field, model::RegisterEntry, variant::Variant};

pub fn rst<'cx>(cr: &mut RegisterEntry<'cx>) {
    let mut rst = cr.add_read_write_field(Field::new("rst", 0, 1));

    rst.add_read_variant(Variant::new("Clear", 0));
    rst.add_read_variant(Variant::new("Set", 1));

    rst.add_write_variant(Variant::new("Noop", 0).inert());
    rst.add_write_variant(Variant::new("Set", 1));
}
