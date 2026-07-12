//! Evaluating phm model descriptions into device models.
//!
//! This is the model-building surface of the build crate — where
//! [`macros`](crate::macros) serves the proc-macro path, this module serves
//! the language path: parse the source (via `proto-hal-syntax`),
//! [elaborate](elaborate) it into a composition, let the model validate
//! itself, and [report] every diagnostic from every phase.
//!
//! [`evaluate`] is the pure entry point, usable by any tool (the `phm` CLI
//! included); [`render`] is the cargo-flavored wrapper for build scripts.

pub mod elaborate;
pub mod report;
pub mod semantic;
pub mod source;

#[cfg(test)]
mod tests;

pub use report::{Diagnostic, Kind, Rank, report};
pub use source::{Sources, load, load_with};

use std::collections::HashMap;

use ::model::Model;
use syntax::ast::File;

/// The outcome of evaluating a model description: the model — whenever one could be
/// built, even alongside errors — and every diagnostic from every phase.
pub struct Evaluation<'src> {
    pub model: Option<Model>,
    pub diagnostics: Vec<Diagnostic<'src>>,
}

impl Evaluation<'_> {
    /// Whether any diagnostic is fatal.
    pub fn failed(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_fatal)
    }
}

/// Evaluate a single-source model description: parse, elaborate, compose,
/// and validate. The source cannot import; for descriptions spanning files,
/// [`load`] and [`evaluate_sources`].
pub fn evaluate(src: &str) -> Evaluation<'_> {
    let (file, errors) = syntax::parse(src, 0);

    let diagnostics = errors
        .into_iter()
        .map(Diagnostic::Syntax)
        .collect::<Vec<_>>();

    let Some(file) = file else {
        return Evaluation {
            model: None,
            diagnostics,
        };
    };

    let units = vec![elaborate::Unit {
        file,
        imports: HashMap::new(),
    }];

    conclude(units, diagnostics)
}

/// Evaluate a loaded model description: parse every source, elaborate,
/// compose, and validate.
pub fn evaluate_sources(sources: &Sources) -> Evaluation<'_> {
    let mut diagnostics = sources
        .issues
        .iter()
        .cloned()
        .map(Diagnostic::Semantic)
        .collect::<Vec<_>>();

    let mut units = Vec::new();
    let mut entry_failed = false;

    for (id, file) in sources.files.iter().enumerate() {
        let (ast, errors) = syntax::parse(&file.content, id);

        diagnostics.extend(errors.into_iter().map(Diagnostic::Syntax));

        entry_failed |= id == 0 && ast.is_none();

        units.push(elaborate::Unit {
            file: ast.unwrap_or(File { items: Vec::new() }),
            imports: file.imports.clone(),
        });
    }

    if entry_failed {
        return Evaluation {
            model: None,
            diagnostics,
        };
    }

    conclude(units, diagnostics)
}

/// Elaborate loaded sources into a [`Composition`] — [`evaluate_sources`]
/// without the model's own validation, for consumers that need the
/// composition itself: the proc-macro path builds gate macros from it.
///
/// Syntax and semantic diagnostics are returned alongside; the composition
/// holds whatever could be built regardless.
pub fn elaborate_sources<'src>(
    sources: &'src Sources,
) -> (::model::Composition, Vec<Diagnostic<'src>>) {
    let mut diagnostics = sources
        .issues
        .iter()
        .cloned()
        .map(Diagnostic::Semantic)
        .collect::<Vec<_>>();

    let mut units = Vec::new();

    for (id, file) in sources.files.iter().enumerate() {
        let (ast, errors) = syntax::parse(&file.content, id);

        diagnostics.extend(errors.into_iter().map(Diagnostic::Syntax));

        units.push(elaborate::Unit {
            file: ast.unwrap_or(File { items: Vec::new() }),
            imports: file.imports.clone(),
        });
    }

    let (composition, semantics, ..) = elaborate::elaborate(&units);
    diagnostics.extend(semantics.into_iter().map(Diagnostic::Semantic));

    (composition, diagnostics)
}

fn conclude<'src>(
    units: Vec<elaborate::Unit<'src>>,
    mut diagnostics: Vec<Diagnostic<'src>>,
) -> Evaluation<'src> {
    let (composition, semantics, locations) = elaborate::elaborate(&units);
    diagnostics.extend(semantics.into_iter().map(Diagnostic::Semantic));

    let (model, model_diagnostics) = composition.finish();
    diagnostics.extend(model_diagnostics.iter().map(|diagnostic| {
        Diagnostic::Semantic(semantic::Diagnostic::model(diagnostic, &locations))
    }));

    Evaluation {
        model: Some(model),
        diagnostics,
    }
}

/// Evaluate the model description at `path` — and everything it imports —
/// and emit generated HAL code: the build-script entry point for
/// `.phm`-described devices.
///
/// On evaluation failure the full diagnostic reports are printed and the
/// build script exits nonzero, which makes cargo display them.
#[cfg(feature = "integrated")]
pub fn render(path: impl AsRef<std::path::Path>) {
    render_with(path, &[]);
}

/// [`render`], with [provided sources](load_with): model component
/// descriptions shipped by dependency crates, satisfying imports before the
/// filesystem.
#[cfg(feature = "integrated")]
pub fn render_with(path: impl AsRef<std::path::Path>, provided: &[(&str, &str)]) {
    let sources = match load_with(path, provided) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    for path in sources.paths() {
        println!("cargo::rerun-if-changed={}", path.display());
    }

    let evaluation = evaluate_sources(&sources);

    if evaluation.failed() {
        report(&sources, &evaluation.diagnostics);
        std::process::exit(1);
    }

    for diagnostic in &evaluation.diagnostics {
        if let Diagnostic::Semantic(semantic) = diagnostic {
            println!("cargo::warning={}", semantic.message);
        }
    }

    let Some(model) = evaluation.model else {
        std::process::exit(1);
    };

    crate::render(&model);
}
