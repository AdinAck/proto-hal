use proto_hal_build::model::structures::{
    entitlement::Entitlement, field::Field, model::RegisterEntry, variant::Variant,
};

pub struct Output {
    pub q15: Entitlement,
    pub q31: Entitlement,
}

pub fn ressize<'cx>(csr: &mut RegisterEntry<'cx>) -> Output {
    let mut ressize = csr
        .add_store_field(Field::new("ressize", 21, 1).docs(["The value format used for results."]));

    Output {
        q31: ressize
            .add_variant(Variant::new("Q31", 0).docs(["1 sign bit, 31 fractional bits."]))
            .make_entitlement(),
        q15: ressize
            .add_variant(Variant::new("Q15", 1).docs(["1 sign bit, 15 fractional bits."]))
            .make_entitlement(),
    }
}
