use std::array;

use proto_hal_model::{Entitlement, Field, Variant, model::RegisterEntry};

pub fn scale<'cx>(csr: &mut RegisterEntry<'cx>) -> [Entitlement; 8] {
    let mut scale = csr.add_store_field(Field::new("scale", 8, 3));

    array::from_fn(|i| {
        scale
            .add_variant(Variant::new(format!("N{i}"), i as _))
            .make_entitlement()
    })
}
