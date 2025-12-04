use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn rev_in<'cx>(cr: &mut RegisterEntry<'cx>) {
    let mut rev_in = cr.add_store_field(Field::new("rev_in", 5, 2));

    rev_in.add_variant(Variant::new("NoEffect", 0));
    rev_in.add_variant(Variant::new("Byte", 1));
    rev_in.add_variant(Variant::new("HalfWord", 2));
    rev_in.add_variant(Variant::new("Word", 3));
}
