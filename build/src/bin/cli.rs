//! The phm command line: evaluate a model description — and everything it
//! imports — report every diagnostic from every phase, and summarize the
//! resulting model.

use proto_hal_build::model::{evaluate_sources, load, report};

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/example.phm"));

    let sources = match load(path) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let evaluation = evaluate_sources(&sources);

    report(&sources, &evaluation.diagnostics);

    if let Some(model) = &evaluation.model {
        println!(
            "\nmodel: {} peripherals, {} registers, {} fields, {} schemas, {} variants, {} interrupts",
            model.peripheral_count(),
            model.register_count(),
            model.field_count(),
            model.schema_count(),
            model.variant_count(),
            model.interrupt_count(),
        );
    }

    if evaluation.failed() {
        std::process::exit(1);
    }
}
