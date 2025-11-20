use proto_hal_build::model::structures::{
    entitlement::Entitlement, field::Field, model::RegisterEntry, variant::Variant,
};

pub struct Output {
    pub one: Entitlement,
}

pub fn nres<'cx>(csr: &mut RegisterEntry) -> Output {
    let mut nres = csr.add_store_field(Field::new("nres", 20, 1));

    let one = nres
        .add_variant(
            Variant::new("One", 0)
                .docs(["One read is needed on the [`rdata`](super::super::rdata) register."]),
        )
        .make_entitlement();
    nres.add_variant(
        Variant::new("Two", 1)
            .docs(["Two reads are needed on the [`rdata`](super::super::rdata) register."]),
    );

    Output { one }
}
