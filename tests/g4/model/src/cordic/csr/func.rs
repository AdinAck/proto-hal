use proto_hal_model::{Entitlement, Field, Variant, error::Error, model::RegisterEntry};

pub struct Entitlements {
    pub n0: Entitlement,
    pub n1: Entitlement,
    pub n2: Entitlement,
    pub n3: Entitlement,
    pub n4: Entitlement,
}

pub fn func<'cx>(csr: &mut RegisterEntry<'cx>, entitlements: Entitlements) -> Result<(), Error> {
    let mut func = csr.add_store_field(Field::new("func", 0, 4));

    let variants = [
        ("cos", vec![entitlements.n0]),
        ("sin", vec![entitlements.n0]),
        ("atan2", vec![entitlements.n0]),
        ("magnitude", vec![entitlements.n0]),
        ("atan", vec![]),
        ("cosh", vec![entitlements.n0]),
        ("sinh", vec![entitlements.n0]),
        ("atanh", vec![entitlements.n0]),
        (
            "ln",
            vec![
                entitlements.n1,
                entitlements.n2,
                entitlements.n3,
                entitlements.n4,
            ],
        ),
        (
            "sqrt",
            vec![entitlements.n0, entitlements.n1, entitlements.n2],
        ),
    ];

    let variants = variants.into_iter().enumerate();

    for (bits, (ident, entitlements)) in variants {
        let mut variant = func.add_variant(Variant::new(ident, bits as _));
        variant.statewise_entitlements([entitlements])?;
    }

    Ok(())
}
