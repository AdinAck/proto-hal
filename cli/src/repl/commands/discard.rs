use crate::repl::Repl;
use clap::Args;
use colored::Colorize;
use ir::utils::diagnostic::Diagnostic;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Discard;

impl Command for Discard {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        *model.hal = model.old_hal.clone();

        println!("{}: discarded pending changes.", "success".green().bold(),);

        Ok(())
    }
}
