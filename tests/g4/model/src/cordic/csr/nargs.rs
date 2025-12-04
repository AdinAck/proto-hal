use proto_hal_build::model::{Entitlement, Field, Variant, model::RegisterEntry};

pub struct Output {
    pub one: Entitlement,
}

pub fn nargs(csr: &mut RegisterEntry) -> Output {
    let mut nargs = csr.add_store_field(Field::new("nargs", 20, 1));

    let one = nargs
        .add_variant(
            Variant::new("One", 0)
                .docs(["One write is needed to the [`wdata`](super::super::wdata) register."]),
        )
        .make_entitlement();
    nargs.add_variant(
        Variant::new("Two", 1)
            .docs(["Two writes are needed to the [`wdata`](super::super::wdata) register."]),
    );

    Output { one }
}
