use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn dmaren<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut dmaren = csr.add_store_field(Field::new("dmaren", 17, 1));

    dmaren.add_variant(Variant::new("Disabled", 0).docs(["No DMA read requests are generated."]));
    dmaren.add_variant(Variant::new("Enabled", 1).docs(["Requests are generated on the DMA read channel whenever the [`rrdy`](super::rrdy) flag is set."]));
}
