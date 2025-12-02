use abstract_model::model;

fn main() {
    proto_hal_build::codegen::render::validate(&model());
}
