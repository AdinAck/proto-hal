use clap::Args;
use ir::utils::diagnostic::Diagnostic;

use crate::repl::Repl;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Exit;

impl Command for Exit {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        model.exit()
    }
}
