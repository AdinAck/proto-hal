use g4_model::{DeviceVariant, model};

fn main() -> phm::Result<()> {
    env_logger::init();
    for variant in [
        DeviceVariant::G431,
        DeviceVariant::G441,
        DeviceVariant::G474,
        DeviceVariant::G484,
    ] {
        println!("=== Variant: {variant:?} ===");
        phm::validate(&model(variant)?);
    }

    Ok(())
}
