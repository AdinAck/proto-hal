use proto_hal_model::{Entitlement, Field, model::RegisterEntry};

pub fn arg0<'cx>(rdata: &mut RegisterEntry<'cx>, q15: Entitlement, nargs_one: Entitlement) {
    let mut arg0 = rdata.add_write_field(Field::new("arg0", 0, 16));
    arg0.ontological_entitlements([q15, nargs_one]);
}
