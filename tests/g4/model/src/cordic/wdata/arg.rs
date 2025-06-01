use proto_hal_build::ir::{
    access::Access,
    structures::field::{Field, Numericity},
};

pub fn generate() -> Field {
    Field::new("arg", 0, 32, Access::write(Numericity::Numeric))
}
