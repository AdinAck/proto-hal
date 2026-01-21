use g4_model::{DeviceVariant, model};
use proto_hal_model::error::Error;

fn main() -> Result<(), Error> {
    env_logger::init();
    for variant in [
        DeviceVariant::G431,
        DeviceVariant::G441,
        DeviceVariant::G474,
        DeviceVariant::G484,
    ] {
        println!("=== Variant: {variant:?} ===");
        proto_hal_model::validate(&model(variant)?);
    }

    Ok(())
}
