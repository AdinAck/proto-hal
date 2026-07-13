//! The phm command line: evaluate model descriptions — and everything they
//! import — report every diagnostic from every phase, and summarize or
//! render the results.

use std::process::ExitCode;

use proto_hal_build::model::{evaluate_sources, load, report, report::rendered};

const USAGE: &str = "\
usage:
  phm check <entries...>         evaluate each device description and report
  phm render <entry> [-o FILE]   emit the generated HAL code
  phm <entries...>               same as `check`";

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    match arguments
        .split_first()
        .map(|(first, rest)| (first.as_str(), rest))
    {
        Some(("check", entries)) if !entries.is_empty() => check(entries),
        Some(("render", arguments)) => render(arguments),
        Some((first, ..)) if !first.starts_with('-') => check(&arguments),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Evaluate each entry, report its diagnostics, and summarize its model.
fn check(entries: &[String]) -> ExitCode {
    let mut failed = false;

    for (index, entry) in entries.iter().enumerate() {
        if entries.len() > 1 {
            if index > 0 {
                println!();
            }

            println!("== {entry}");
        }

        let sources = match load(entry) {
            Ok(sources) => sources,
            Err(e) => {
                eprintln!("{e}");
                failed = true;
                continue;
            }
        };

        let evaluation = evaluate_sources(&sources);

        report(&sources, &evaluation.diagnostics);

        if let Some(model) = &evaluation.model {
            println!(
                "model: {} peripherals, {} registers, {} fields, {} schemas, {} variants, {} interrupts",
                model.peripheral_count(),
                model.register_count(),
                model.field_count(),
                model.schema_count(),
                model.variant_count(),
                model.interrupt_count(),
            );
        }

        failed |= evaluation.failed();
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Evaluate one entry and emit its generated HAL code — to stdout, or to
/// the file named by `-o`. Diagnostics go to stderr, so piped output stays
/// pure code.
fn render(arguments: &[String]) -> ExitCode {
    let (entry, output) = match arguments {
        [entry] => (entry, None),
        [entry, flag, path] if flag == "-o" => (entry, Some(path)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let sources = match load(entry) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let evaluation = evaluate_sources(&sources);

    for diagnostic in &evaluation.diagnostics {
        eprint!("{}", rendered(&sources, diagnostic));
    }

    if evaluation.failed() {
        return ExitCode::FAILURE;
    }

    let Some(model) = evaluation.model else {
        return ExitCode::FAILURE;
    };

    let code = match model.render() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("the generated code does not parse: {e}");
            return ExitCode::FAILURE;
        }
    };

    match output {
        Some(path) => {
            if let Err(e) = std::fs::write(path, code) {
                eprintln!("cannot write `{path}`: {e}");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        None => {
            print!("{code}");
            ExitCode::SUCCESS
        }
    }
}
