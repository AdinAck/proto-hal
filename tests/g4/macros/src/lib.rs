use model::DeviceVariant;
use proto_hal_macros::generate_macros;

generate_macros!({
    #[cfg(feature = "g431")]
    let variant = DeviceVariant::G431;
    #[cfg(feature = "g441")]
    let variant = DeviceVariant::G441;
    #[cfg(feature = "g474")]
    let variant = DeviceVariant::G474;
    #[cfg(feature = "g484")]
    let variant = DeviceVariant::G484;

    variant
});
