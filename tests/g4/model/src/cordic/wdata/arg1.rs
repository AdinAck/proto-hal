use proto_hal_build::model::{
    access::Access,
    structures::{
        entitlement::Entitlement,
        field::{Field, Numericity},
    },
};

pub fn generate() -> Field {
    Field::new("arg1", 16, 16, Access::write(Numericity::Numeric)).entitlements([
        Entitlement::to("cordic::csr::argsize::Q15"),
        Entitlement::to("cordic::csr::nargs::One"),
    ])
}
