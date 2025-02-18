use crate::repl::Repl;
use clap::Args;
use ir::utils::diagnostic::Diagnostic;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Diff;

impl Command for Diff {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        todo!()
    }
}
