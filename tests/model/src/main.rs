use colored::Colorize as _;
use proto_hal_build::ir::utils::diagnostic::{self, Diagnostic};

fn main() {
    if let Err(diagnostics) = model::generate() {
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
    } else {
        println!("{} with 0 errors and 0 warnings.", "Done".green().bold());
    }
}
