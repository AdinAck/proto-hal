use proto_hal_build::model::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("arg", 0, 32, Access::write(Numericity::Numeric))
        .entitlements([Entitlement::to("cordic::csr::argsize::Q31")])
}
