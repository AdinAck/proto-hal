use proto_hal_build::ir::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("resq31", 0, 32, Access::read(Numericity::Numeric))
        .entitlements([Entitlement::to("cordic::csr::ressize::Q31")])
}
