use proto_hal_build::ir::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("arg0", 0, 16, Access::write(Numericity::Numeric))
        .entitlements([Entitlement::to("cordic::csr::argsize::Q15")])
}
