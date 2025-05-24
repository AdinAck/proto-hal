fn main() {
    env_logger::init();
    proto_hal_build::codegen::validate(g4_model::generate);
}
