//! The phm command line: evaluate model descriptions — and everything they
//! import — report every diagnostic from every phase, and summarize or
//! render the results.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use proto_hal_build::model::{evaluate_sources, load, report, report::rendered, root};

const USAGE: &str = "\
usage:
  phm check [entries...]         evaluate device descriptions and report — a
                                 directory, or nothing, discovers the entries
                                 in devices/ beside the nearest phm.toml
  phm render <entry> [-o FILE]   emit the generated HAL code
  phm lsp                        serve the language protocol over stdio
  phm <entries...>               same as `check`";

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();

    match arguments
        .split_first()
        .map(|(first, rest)| (first.as_str(), rest))
    {
        Some(("check", entries)) => check(entries),
        Some(("render", arguments)) => render(arguments),
        Some(("lsp", [])) => lsp(),
        Some((first, ..)) if !first.starts_with('-') => check(&arguments),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Evaluate each entry, report its diagnostics, and summarize its model.
///
/// A directory argument — or none, standing for the working directory —
/// names a model by its `phm.toml`: every `.phm` beside the manifest is an
/// entry.
fn check(arguments: &[String]) -> ExitCode {
    let mut entries = Vec::new();
    let mut failed = false;

    if arguments.is_empty() {
        match discovered(Path::new(".")) {
            Ok(found) => entries.extend(found),
            Err(message) => {
                eprintln!("{message}");
                return ExitCode::FAILURE;
            }
        }
    }

    for argument in arguments {
        let path = Path::new(argument);

        if path.is_dir() {
            match discovered(path) {
                Ok(found) => entries.extend(found),
                Err(message) => {
                    eprintln!("{message}");
                    failed = true;
                }
            }
        } else {
            entries.push(path.to_path_buf());
        }
    }

    for (index, entry) in entries.iter().enumerate() {
        if entries.len() > 1 {
            if index > 0 {
                println!();
            }

            println!("== {}", entry.display());
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

/// The device entries of the model governing `from`: the `.phm` files in
/// `devices/` beside the nearest `phm.toml`.
fn discovered(from: &Path) -> Result<Vec<PathBuf>, String> {
    let root = root(from).ok_or_else(|| {
        format!(
            "no `phm.toml` marks a model at or above `{}`",
            from.display(),
        )
    })?;

    let devices = root.join("devices");

    let mut entries = std::fs::read_dir(&devices)
        .map_err(|e| format!("cannot read `{}`: {e}", devices.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "phm"))
        .collect::<Vec<_>>();

    entries.sort();

    if entries.is_empty() {
        return Err(format!(
            "the model at `{}` has no device descriptions",
            root.display(),
        ));
    }

    Ok(entries)
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

/// Run the language server over stdio.
#[cfg(feature = "lsp")]
fn lsp() -> ExitCode {
    match proto_hal_build::model::lsp::serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(feature = "lsp"))]
fn lsp() -> ExitCode {
    eprintln!("this `phm` was built without the language server — rebuild with `--features lsp`");
    ExitCode::FAILURE
}
