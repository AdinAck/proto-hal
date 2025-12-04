use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn precision<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut precision = csr.add_store_field(Field::new("precision", 4, 4));

    let variants = (1..16).map(|i| Variant::new(format!("P{}", i * 4), i));

    for variant in variants {
        precision.add_variant(variant);
    }
}
