use g4_model::DeviceVariant;

fn main() {
    env_logger::init();
    for variant in [
        DeviceVariant::G431,
        DeviceVariant::G441,
        DeviceVariant::G474,
        DeviceVariant::G484,
    ] {
        println!("=== Variant: {variant:?} ===");
        proto_hal_build::codegen::validate(|| g4_model::generate(variant));
    }
}
