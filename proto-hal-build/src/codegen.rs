use std::{env, fs, path::Path};

use colored::Colorize as _;
use ir::{
    structures::hal::Hal,
    utils::diagnostic::{self, Diagnostic, Diagnostics},
};

/// Validate a HAL model is properly defined and codegen succeeds.
///
/// *Note: This function is intended to be called in the "model" phase of synthesis.*
pub fn validate(source: impl FnOnce() -> Result<Hal, Diagnostics>) {
    // model validation
    println!("Validating model...");
    let hal = match source() {
        Ok(hal) => hal,
        Err(diagnostics) => {
            println!("{}", Diagnostic::report(&diagnostics));

            let warning_count = diagnostics
                .iter()
                .filter(|diagnostic| matches!(diagnostic.kind(), diagnostic::Kind::Warning))
                .count();

            let error_count = diagnostics
                .iter()
                .filter(|diagnostic| matches!(diagnostic.kind(), diagnostic::Kind::Error))
                .count();

            println!("emitted {warning_count} warnings and {error_count} errors.");
            return;
        }
    };
    println!("{} with 0 errors and 0 warnings.", "Done".green().bold());

    // codegen validation
    println!("Validating codegen...");
    match hal.render() {
        Ok(output) => {
            println!(
                "{}. Produced {} lines.",
                "Done".green().bold(),
                output.lines().count()
            );
        }
        Err(e) => {
            println!(
                "{}: Codegen failed: {e}.\n{}",
                "error".red().bold(),
                "This is probably a bug, please submit an issue: https://github.com/adinack/proto-hal/issues".bold(),
            );
        }
    }
}

/// Generate and emit HAL code for use.
///
/// *Note: This function is intended to be called in the "out" phase of synthesis.*
pub fn generate(source: impl FnOnce() -> Result<Hal, Diagnostics>) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("hal.rs");

    let Ok(hal) = source() else {
        println!("cargo::error=HAL generation failed. Refer to the model crate for details.");
        return;
    };

    let Ok(codegen) = hal.render() else {
        println!("cargo::error=Codegen failed. Refer to the model crate for details.");
        return;
    };

    fs::write(&dest_path, codegen).unwrap();

    // device.x
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("device.x");

    fs::write(&dest_path, hal.interrupts.device_x()).unwrap();

    println!("cargo:rustc-link-search={}", out_dir);
}
