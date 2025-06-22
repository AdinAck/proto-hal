use model::DeviceVariant;

fn main() -> Result<(), String> {
    let variant = if cfg!(feature = "g431") {
        DeviceVariant::G431
    } else if cfg!(feature = "g441") {
        DeviceVariant::G441
    } else if cfg!(feature = "g474") {
        DeviceVariant::G474
    } else if cfg!(feature = "g484") {
        DeviceVariant::G484
    } else {
        Err("device variant must be specified")?
    };

    proto_hal_build::codegen::generate(|| model::generate(variant));

    Ok(())
}
