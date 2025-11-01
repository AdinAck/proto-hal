use std::fs;

use colored::Colorize as _;
use ir::{
    diagnostic::{self, Diagnostic},
    structures::{hal::Hal, interrupts::InterruptKind},
};

/// Validate a HAL model is properly defined and codegen succeeds.
///
/// *Note: This function is intended to be called in the "model" phase of synthesis.*
pub fn validate(hal: &Hal) {
    // model validation
    println!("Validating model...");
    let diagnostics = hal.validate();

    if !diagnostics.is_empty() {
        println!("{}", Diagnostic::report(&diagnostics));
    }

    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.kind(), diagnostic::Kind::Warning))
        .count();

    let error_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.kind(), diagnostic::Kind::Error))
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
            let peripherals = hal.peripherals.len();
            let registers = hal
                .peripherals
                .values()
                .map(|peripheral| peripheral.registers.len())
                .sum::<usize>();
            let fields = hal
                .peripherals
                .values()
                .flat_map(|peripheral| peripheral.registers.values())
                .map(|register| register.fields.len())
                .sum::<usize>();
            let interrupts = hal.interrupts.len();
            let reserved_interrupts = hal
                .interrupts
                .iter()
                .filter(|interrupt| matches!(interrupt.kind, InterruptKind::Reserved))
                .count();

            println!(
                "Peripherals: {peripherals}\nRegisters: {registers}\nFields: {fields}\nInterrupts: {interrupts} ({reserved_interrupts} reserved)\nLines: {}\n{}",
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
pub fn generate(hal: &Hal) {
    super::generate(hal, |hal| {
        Ok([
            ("hal.rs".to_string(), hal.render()?),
            ("device.x".to_string(), hal.interrupts.device_x()),
        ]
        .into())
    });
}
