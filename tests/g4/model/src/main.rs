use g4_model::{DeviceVariant, model};

fn main() {
    env_logger::init();
    for variant in [
        DeviceVariant::G431,
        DeviceVariant::G441,
        DeviceVariant::G474,
        DeviceVariant::G484,
    ] {
        println!("=== Variant: {variant:?} ===");
        proto_hal_build::codegen::render::validate(&model(variant));
    }
}
