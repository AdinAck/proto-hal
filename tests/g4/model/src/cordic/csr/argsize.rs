use proto_hal_build::model::{Entitlement, Field, Variant, model::RegisterEntry};

pub struct Output {
    pub q15: Entitlement,
    pub q31: Entitlement,
}

pub fn argsize<'cx>(csr: &mut RegisterEntry<'cx>) -> Output {
    let mut argsize = csr.add_store_field(
        Field::new("argsize", 22, 1).docs(["The value format used for arguments."]),
    );

    Output {
        q31: argsize
            .add_variant(Variant::new("Q31", 0).docs(["1 sign bit, 31 fractional bits."]))
            .make_entitlement(),
        q15: argsize
            .add_variant(Variant::new("Q15", 1).docs(["1 sign bit, 15 fractional bits."]))
            .make_entitlement(),
    }
}
