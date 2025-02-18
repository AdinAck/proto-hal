use std::{collections::HashMap, fmt::Display};

use colored::Colorize;
use ters::ters;

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct Diagnostic {
    message: String,
    #[get]
    kind: Kind,
    context: Option<Context>,
}

impl Diagnostic {
    pub fn warning(message: String) -> Self {
        Self {
            message,
            kind: Kind::Warning,
            context: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            message,
            kind: Kind::Error,
            context: None,
        }
    }

    pub fn with_context(mut self, context: Context) -> Self {
        self.context = Some(context);
        self
    }

    pub fn report(diagnostics: &Vec<Self>) -> String {
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
        match &self.kind {
            Kind::Warning => write!(f, "{}: {}", "warning".yellow().bold(), self.message),
            Kind::Error => write!(f, "{}: {}", "error".red().bold(), self.message),
        }
    }
}

pub type Diagnostics = Vec<Diagnostic>;
