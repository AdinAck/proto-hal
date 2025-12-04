use std::fs;

use colored::Colorize as _;
use model::{
    diagnostic::{self, Diagnostic},
    {Model, interrupts::InterruptKind},
};

/// Validate a HAL model is properly defined and codegen succeeds.
///
/// *Note: This function is intended to be called in the "model" phase of synthesis.*
pub fn validate(hal: &Model) {
    // model validation
    println!("Validating model...");
    let diagnostics = hal.validate();

    if !diagnostics.is_empty() {
        println!("{}", Diagnostic::report(&diagnostics));
    }

    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.rank(), diagnostic::Rank::Warning))
        .count();

    let error_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.rank(), diagnostic::Rank::Error))
        .count();

    if error_count == 0 {
        print!("{}. ", "Finished".green().bold());
    }
    println!("emitted {warning_count} warnings and {error_count} errors");

    if error_count != 0 {
        return;
    }

    // codegen validation
    println!("Validating codegen...");
    match hal.render() {
        Ok(output) => {
            let reserved_interrupts = hal
                .interrupts()
                .iter()
                .filter(|interrupt| matches!(interrupt.kind, InterruptKind::Reserved))
                .count();

            println!(
                "Peripherals: {}\nRegisters: {}\nFields: {}\nInterrupts: {} ({reserved_interrupts} reserved)\nLines: {}\n{}",
                hal.peripheral_count(),
                hal.register_count(),
                hal.field_count(),
                hal.interrupt_count(),
                output.lines().count(),
                "Finished".green().bold(),
            );
        }
        Err(e) => {
            fs::write("/tmp/erroneous-hal.rs", hal.render_raw()).unwrap();

            println!(
                "{}: Codegen failed: {e}\n{}\nErroneous codegen written to /tmp/erroneous-hal.rs",
                "error".red().bold(),
                "This is probably a bug, please submit an issue: https://github.com/adinack/proto-hal/issues".bold(),
            );
        }
    }
}

/// Generate and emit HAL code for use.
///
/// *Note: This function is intended to be called in the "out" phase of synthesis.*
pub fn generate(hal: &Model) {
    super::generate(hal, |hal| {
        Ok([
            ("hal.rs".to_string(), hal.render()?),
            ("device.x".to_string(), hal.interrupts().device_x()),
        ]
        .into())
    });
}
