use crate::repl::Repl;
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Diff;

impl Command for Diff {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        todo!()
    }
}
