use proto_hal_model::{Entitlement, Field, model::RegisterEntry};

pub fn arg1<'cx>(rdata: &mut RegisterEntry<'cx>, q15: Entitlement, nargs_one: Entitlement) {
    let mut arg1 = rdata.add_write_field(Field::new("arg1", 16, 16));
    arg1.ontological_entitlements([q15, nargs_one]);
}
