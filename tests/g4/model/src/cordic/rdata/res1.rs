use proto_hal_build::model::structures::{
    entitlement::Entitlement, field::Field, model::RegisterEntry,
};

pub fn res1<'cx>(rdata: &mut RegisterEntry<'cx>, q15: Entitlement, nres_one: Entitlement) {
    let mut res1 = rdata.add_read_field(Field::new("res1", 16, 16));
    res1.ontological_entitlements([q15, nres_one]);
}
