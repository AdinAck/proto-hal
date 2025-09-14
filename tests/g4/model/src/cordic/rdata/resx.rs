use proto_hal_build::ir::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("resx", 0, 16, Access::read(Numericity::Numeric))
        .entitlements([
            Entitlement::to("cordic::csr::ressize::Q15"),
            Entitlement::to("cordic::csr::nres::One"),
        ])
        .array(2, |i| format!("res{i}"))
}
