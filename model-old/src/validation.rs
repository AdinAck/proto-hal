use std::{fs, process::ExitCode};

use colored::Colorize as _;

use crate::{
    Composition,
    diagnostic::{self, Diagnostic},
    interrupts::InterruptKind,
};

/// Validate a HAL model is properly defined and codegen succeeds.
pub fn validate(composition: Composition) -> ExitCode {
    // model validation
    println!("Validating model...");
    let (model, diagnostics) = composition.finish();

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
        return ExitCode::FAILURE;
    }

    // codegen validation
    println!("Validating codegen...");
    match model.render() {
        Ok(output) => {
            let reserved_interrupts = model
                .interrupts()
                .iter()
                .filter(|interrupt| matches!(interrupt.kind, InterruptKind::Reserved))
                .count();

            println!(
                "Peripherals: {}\nRegisters: {}\nFields: {}\nInterrupts: {} ({reserved_interrupts} reserved)\nLines: {}\n{}",
                model.peripheral_count(),
                model.register_count(),
                model.field_count(),
                model.interrupt_count(),
                output.lines().count(),
                "Finished".green().bold(),
            );

            ExitCode::SUCCESS
        }
        Err(e) => {
            fs::write("/tmp/erroneous-hal.rs", model.render_raw()).unwrap();

            println!(
                "{}: Codegen failed: {e}\n{}\nErroneous codegen written to /tmp/erroneous-hal.rs",
                "error".red().bold(),
                "This is probably a bug, please submit an issue: https://github.com/adinack/proto-hal/issues".bold(),
            );

            ExitCode::FAILURE
        }
    }
}
