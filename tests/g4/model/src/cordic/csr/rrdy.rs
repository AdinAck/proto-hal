use proto_hal_build::model::{Field, Variant, model::RegisterEntry};

pub fn rrdy<'cx>(csr: &mut RegisterEntry<'cx>) {
    let mut rrdy = csr.add_read_field(Field::new("rrdy", 31, 1));

    rrdy.add_variant(Variant::new("NoData", 0).docs(["No new data in output register."]));
    rrdy.add_variant(
        Variant::new("Ready", 1).docs(["[`rdata`](super::super::rdata) contains a result."]),
    );
}
