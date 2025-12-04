use proto_hal_model::{Entitlement, Field, model::RegisterEntry};

pub fn res0<'cx>(rdata: &mut RegisterEntry<'cx>, q15: Entitlement, nres_one: Entitlement) {
    let mut res0 = rdata.add_read_field(Field::new("res0", 0, 16));
    res0.ontological_entitlements([q15, nres_one]);
}
