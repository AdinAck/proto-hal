use proto_hal_build::model::structures::{
    entitlement::Entitlement, field::Field, model::RegisterEntry,
};

pub fn arg<'cx>(wdata: &mut RegisterEntry<'cx>, q31: Entitlement) {
    let mut arg = wdata.add_write_field(Field::new("arg", 0, 32));
    arg.ontological_entitlements([q31]);
}
