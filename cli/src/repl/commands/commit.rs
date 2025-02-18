use std::fs;

use crate::repl::Repl;
use clap::Args;
use colored::Colorize;
use ir::utils::diagnostic::Diagnostic;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Commit;

impl Command for Commit {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        fs::write(
            model.file,
            toml::to_string_pretty(model.hal).map_err(|e| Diagnostic::error(e.to_string()))?,
        )
        .map_err(|e| Diagnostic::error(e.to_string()))?;
        model.old_hal = model.hal.clone();

        println!(
            "{}: wrote changes to [{}].",
            "success".green().bold(),
            model.file.to_str().unwrap().bold()
        );

        Ok(())
    }
}
