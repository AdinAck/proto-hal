use proto_hal_build::model::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("res", 0, 32, Access::read(Numericity::Numeric))
        .entitlements([Entitlement::to("cordic::csr::ressize::Q31")])
}
