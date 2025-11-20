use proto_hal_build::model::structures::{field::Field, model::RegisterEntry, variant::Variant};

pub fn rev_out<'cx>(cr: &mut RegisterEntry<'cx>) {
    let mut rev_out = cr.add_store_field(Field::new("rev_in", 7, 1));

    rev_out.add_variant(Variant::new("NoEffect", 0));
    rev_out.add_variant(Variant::new("Reversed", 1));
}
