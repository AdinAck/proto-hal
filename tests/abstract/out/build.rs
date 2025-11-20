use model::model;

fn main() {
    proto_hal_build::codegen::render::generate(&model());
}
