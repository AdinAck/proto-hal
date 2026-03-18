use phm::{Entitlement, Field, model::RegisterEntry};

pub fn res<'cx>(rdata: &mut RegisterEntry<'cx>, q31: Entitlement) -> phm::Result<()> {
    let mut res = rdata.add_read_field(Field::new("res", 0, 32));
    res.ontological_entitlements([[q31]])?;

    Ok(())
}
