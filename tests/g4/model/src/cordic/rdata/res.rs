use proto_hal_build::ir::{
    access::Access,
    structures::field::{Field, Numericity},
};

pub fn generate() -> Field {
    Field::new("res", 0, 32, Access::read(Numericity::Numeric))
}
