use std::{collections::HashMap, fmt::Display};

use colored::Colorize;

pub struct Warning(pub String);
pub struct Error(pub String);

impl Warning {
    pub fn with_context(self, context: Context) -> Diagnostic {
        Diagnostic {
            kind: Kind::Warning(self),
            context,
        }
    }
}

impl Error {
    pub fn with_context(self, context: Context) -> Diagnostic {
        Diagnostic {
            kind: Kind::Error(self),
            context,
        }
    }
}

pub enum Kind {
    Warning(Warning),
    Error(Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    path: Vec<String>,
}

impl Context {
    pub fn new() -> Self {
        Context { path: Vec::new() }
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

pub struct Diagnostic {
    kind: Kind,
    context: Context,
}

impl Diagnostic {
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
                format!(
                    "in {}:\n{}",
                    context,
                    diagnostics
                        .iter()
                        .map(|diagnostic| match &diagnostic.kind {
                            Kind::Warning(warning) =>
                                format!("{}: {}", "warning".yellow().bold(), warning.0),
                            Kind::Error(error) => format!("{}: {}", "error".red().bold(), error.0),
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
