//! The unified diagnostic stream, rendered against the source text.
//!
//! There are four phases of diagnostics, in pipeline order:
//! 1. **lexical** — the source couldn't be tokenized (rare; unrecognized
//!    input lexes as tokens),
//! 2. **syntax** — the tokens don't form the grammar,
//! 3. **semantic** — the tree is well-formed but doesn't make sense, judged
//!    during [elaboration](super::elaborate),
//! 4. **model** — the composed model judges *itself*: overlap, alignment,
//!    reset coverage, and friends, emitted by the model crate.
//!
//! Every diagnostic carries a code from the one shared catalog,
//! [`Kind`] — rendered as `error[Exxxx]`/`warning[Wxxxx]` across all phases,
//! by one [renderer](render), anchored to source spans. Model diagnostics
//! arrive here already converted (see [`semantic::Diagnostic::model`]).
//!
//! The diagnostics themselves — every message, label, and note — live in
//! [`semantic`], one catalog to read top to bottom.
//!
//! [`semantic`]: super::semantic
//! [`semantic::Diagnostic::model`]: super::semantic::Diagnostic::model

mod render;

use syntax::{Rich, ast::Span};

pub use ::model::diagnostic::{Kind, Rank};

use crate::model::{
    report::render::render,
    semantic,
    source::Sources,
};

pub(crate) use render::Rendering;

/// A diagnostic from any phase of model evaluation.
#[derive(Debug)]
pub enum Diagnostic<'src> {
    /// Lexical or syntactic: the source doesn't form the language.
    Syntax(syntax::Error<'src>),
    /// Semantic: the tree doesn't make sense — judged during elaboration, or
    /// by the model itself (anchored back to the text on the way here).
    Semantic(semantic::Diagnostic),
}

impl Diagnostic<'_> {
    /// Whether this diagnostic should fail evaluation (warnings do not).
    pub fn is_fatal(&self) -> bool {
        match self {
            Self::Syntax(..) => true,
            Self::Semantic(semantic) => matches!(semantic.rank, Rank::Error),
        }
    }
}

impl<'src> From<syntax::Error<'src>> for Diagnostic<'src> {
    fn from(error: syntax::Error<'src>) -> Self {
        Self::Syntax(error)
    }
}

impl From<semantic::Diagnostic> for Diagnostic<'_> {
    fn from(semantic: semantic::Diagnostic) -> Self {
        Self::Semantic(semantic)
    }
}

/// Render the provided diagnostics against their source files: every span
/// carries its [`SourceId`](syntax::ast::SourceId), so reports may reach into
/// any loaded file — or several at once.
pub fn report(sources: &Sources, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        print!("{}", rendered(sources, diagnostic));
    }
}

/// One diagnostic, rendered to a string.
pub fn rendered(sources: &Sources, diagnostic: &Diagnostic) -> String {
    render(sources, &rendering(diagnostic))
}

/// One diagnostic, shaped for presentation: message, labels — the primary
/// first — and notes. The terminal renderer and the language server both
/// present this.
pub(crate) fn rendering<'a>(diagnostic: &'a Diagnostic) -> Rendering<'a> {
    match diagnostic {
        Diagnostic::Syntax(syntax::Error::Lex(error)) => rich(Kind::Lexical, error),
        Diagnostic::Syntax(syntax::Error::Parse(error)) => rich(Kind::Syntax, error),
        Diagnostic::Semantic(semantic) => Rendering {
            kind: semantic.kind,
            rank: semantic.rank.clone(),
            message: semantic.message.clone(),
            labels: std::iter::once((semantic.span, semantic.label.clone(), true))
                .chain(
                    semantic
                        .labels
                        .iter()
                        .map(|(span, message)| (*span, message.clone(), false)),
                )
                .collect(),
            notes: &semantic.notes,
        },
    }
}

/// A chumsky error, shaped for the renderer: the reason is the primary
/// label, parse contexts become secondary labels.
fn rich<'a, T>(kind: Kind, error: &'a Rich<'a, T, Span>) -> Rendering<'a>
where
    T: std::fmt::Display,
{
    Rendering {
        kind,
        rank: Rank::Error,
        message: error.to_string(),
        labels: std::iter::once((*error.span(), error.reason().to_string(), true))
            .chain(
                error
                    .contexts()
                    .map(|(label, span)| (*span, format!("while parsing this {label}"), false)),
            )
            .collect(),
        notes: &[],
    }
}
