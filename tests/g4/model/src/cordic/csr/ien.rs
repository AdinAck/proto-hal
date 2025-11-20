use proto_hal_build::model::structures::{field::Field, model::RegisterEntry, variant::Variant};

pub fn ien<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut ien = csr.add_store_field(Field::new("ien", 16, 1));

    ien.add_variant(Variant::new("Disabled", 0).docs(["No interrupt requests generated."]));
    ien.add_variant(Variant::new("Enabled", 1).docs([
        "An interrupt request is generated whenever the [`rrdy`](super::rrdy) flag is set.",
    ]));
}
