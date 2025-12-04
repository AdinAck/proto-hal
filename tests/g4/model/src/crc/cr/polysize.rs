use proto_hal_model::{Field, Variant, model::RegisterEntry};

pub fn polysize<'cx>(cr: &mut RegisterEntry<'cx>) {
    let mut polysize = cr.add_store_field(Field::new("polysize", 3, 2));

    polysize.add_variant(Variant::new("P32", 0));
    polysize.add_variant(Variant::new("P16", 1));
    polysize.add_variant(Variant::new("P8", 2));
    polysize.add_variant(Variant::new("P7", 3));
}
