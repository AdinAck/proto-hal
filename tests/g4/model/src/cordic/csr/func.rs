use proto_hal_build::model::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
        variant::Variant,
    },
};

pub fn generate() -> Field {
    let scale = |variant: &str| format!("cordic::csr::scale::{variant}");

    let variants = [
        ("cos", vec![Entitlement::to(scale("N0"))]),
        ("sin", vec![Entitlement::to(scale("N0"))]),
        ("atan2", vec![Entitlement::to(scale("N0"))]),
        ("magnitude", vec![Entitlement::to(scale("N0"))]),
        ("atan", vec![]),
        ("cosh", vec![Entitlement::to(scale("N0"))]),
        ("sinh", vec![Entitlement::to(scale("N0"))]),
        ("atanh", vec![Entitlement::to(scale("N0"))]),
        (
            "ln",
            vec![
                Entitlement::to(scale("N1")),
                Entitlement::to(scale("N2")),
                Entitlement::to(scale("N3")),
                Entitlement::to(scale("N4")),
            ],
        ),
        (
            "sqrt",
            vec![
                Entitlement::to(scale("N0")),
                Entitlement::to(scale("N1")),
                Entitlement::to(scale("N2")),
            ],
        ),
    ];

    let variants = variants
        .into_iter()
        .enumerate()
        .map(|(bits, (ident, entitlements))| {
            Variant::new(ident, bits as u32).entitlements(entitlements)
        });

    Field::new(
        "func",
        0,
        4,
        Access::read_write(Numericity::enumerated(variants)),
    )
}
