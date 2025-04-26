use colored::Colorize as _;
use proto_hal_build::ir::utils::diagnostic::{self, Diagnostic};

fn main() {
    // model validation
    println!("Validating model...");
    let hal = match model::generate() {
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
    if let Err(e) = hal.render() {
        println!(
            "{}: Codegen failed: {e}.\nThis is probably a bug, please submit an issue.",
            "error".red().bold()
        );
    } else {
        println!("{}", "Done".green().bold());
    }
}
