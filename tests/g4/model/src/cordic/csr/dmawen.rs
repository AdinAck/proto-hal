use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn dmawen<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut dmaren = csr.add_store_field(Field::new("dmawen", 18, 1));

    dmaren.add_variant(Variant::new("Disabled", 0).docs(["No DMA write requests are generated."]));
    dmaren.add_variant(Variant::new("Enabled", 1).docs([
        "Requests are generated on the DMA write channel whenever no operation is pending.",
    ]));
}
