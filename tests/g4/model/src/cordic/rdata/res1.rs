use proto_hal_build::model::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("res1", 16, 16, Access::read(Numericity::Numeric)).entitlements([
        Entitlement::to("cordic::csr::ressize::Q15"),
        Entitlement::to("cordic::csr::nres::One"),
    ])
}
