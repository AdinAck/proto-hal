use proto_hal_build::ir::{structures::hal::Hal, utils::diagnostic::Diagnostics};

pub mod cordic;
pub mod rcc;

pub fn generate() -> Result<Hal, Diagnostics> {
    let hal = Hal::new([rcc::generate(), cordic::generate()]);

    let diagnostics = hal.validate();

    if diagnostics.len() > 0 {
        Err(diagnostics)?
    }

    Ok(hal)
}
