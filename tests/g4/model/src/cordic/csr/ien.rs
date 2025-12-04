use proto_hal_model::{Field, Variant, model::RegisterEntry};

pub fn ien<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut ien = csr.add_store_field(Field::new("ien", 16, 1));

    ien.add_variant(Variant::new("Disabled", 0).docs(["No interrupt requests generated."]));
    ien.add_variant(Variant::new("Enabled", 1).docs([
        "An interrupt request is generated whenever the [`rrdy`](super::rrdy) flag is set.",
    ]));
}
