use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn rev_out<'cx>(cr: &mut RegisterEntry<'cx>) {
    let mut rev_out = cr.add_store_field(Field::new("rev_out", 7, 1));

    rev_out.add_variant(Variant::new("NoEffect", 0));
    rev_out.add_variant(Variant::new("Reversed", 1));
}
