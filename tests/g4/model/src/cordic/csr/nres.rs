use proto_hal_model::{Entitlement, Field, Variant, model::RegisterEntry};

pub struct Output {
    pub one: Entitlement,
}

pub fn nres(csr: &mut RegisterEntry) -> Output {
    let mut nres = csr.add_store_field(Field::new("nres", 19, 1));

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
