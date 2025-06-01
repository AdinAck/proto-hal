use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use colored::Colorize;
use ters::ters;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Kind {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    path: Vec<String>,
}

impl Context {
    pub fn new() -> Self {
        Context { path: Vec::new() }
    }

    pub fn with_path(path: Vec<String>) -> Self {
        Self { path }
    }

    pub fn and(mut self, ident: String) -> Self {
        self.path.push(ident);
        self
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.path
                .iter()
                .map(|segment| segment.bold().to_string())
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}

#[ters]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Diagnostic {
    #[get]
    message: String,
    notes: Vec<String>,
    #[get]
    kind: Kind,
    #[get]
    context: Option<Context>,
}

impl Diagnostic {
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            notes: Vec::new(),
            kind: Kind::Warning,
            context: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            notes: Vec::new(),
            kind: Kind::Error,
            context: None,
        }
    }

    pub fn with_context(mut self, context: Context) -> Self {
        self.context = Some(context);
        self
    }

    pub fn notes<I>(mut self, notes: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.notes
            .extend(notes.into_iter().map(|e| e.as_ref().to_string()));

        self
    }

    pub fn report(diagnostics: &HashSet<Self>) -> String {
        let mut diagnostic_groups = HashMap::new();

        for diagnostic in diagnostics {
            diagnostic_groups
                .entry(diagnostic.context.clone())
                .or_insert(vec![])
                .push(diagnostic);
        }

        diagnostic_groups
            .iter()
            .map(|(context, diagnostics)| {
                let diagnostics = diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                if let Some(context) = context {
                    format!("in {}:\n{}", context, diagnostics)
                } else {
                    format!("{}", diagnostics)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let notes = if !self.notes.is_empty() {
            format!(
                "\n{}",
                self.notes
                    .iter()
                    .map(|note| format!("\t{}: {note}", "note".bright_blue().bold()))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            "".to_string()
        };
        let kind = match &self.kind {
            Kind::Warning => "warning".yellow().bold(),
            Kind::Error => "error".red().bold(),
        };

        write!(f, "{kind}: {}{notes}", self.message)
    }
}

pub type Diagnostics = HashSet<Diagnostic>;

impl From<Diagnostic> for Diagnostics {
    fn from(diagnostic: Diagnostic) -> Self {
        HashSet::from([diagnostic])
    }
}
