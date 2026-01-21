pub mod diagnostic;
pub mod entitlement;
pub mod error;
pub mod field;
pub mod interrupts;
pub mod model;
pub mod peripheral;
pub mod register;
pub mod variant;

use std::fs;

use colored::Colorize as _;
pub use entitlement::Entitlement;
pub use field::Field;
pub use interrupts::{Interrupt, Interrupts};
pub use model::Model;
pub use peripheral::Peripheral;
pub use register::Register;
pub use variant::Variant;

use crate::{diagnostic::Diagnostic, interrupts::InterruptKind};

#[doc(hidden)]
pub trait Node {
    type Index;
}

/// Validate a HAL model is properly defined and codegen succeeds.
pub fn validate(model: &Model) {
    // model validation
    println!("Validating model...");
    let diagnostics = model.validate();

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
        }
        Err(e) => {
            fs::write("/tmp/erroneous-hal.rs", model.render_raw()).unwrap();

            println!(
                "{}: Codegen failed: {e}\n{}\nErroneous codegen written to /tmp/erroneous-hal.rs",
                "error".red().bold(),
                "This is probably a bug, please submit an issue: https://github.com/adinack/proto-hal/issues".bold(),
            );
        }
    }
}
