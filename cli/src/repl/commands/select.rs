use crate::{repl::Repl, utils::path::Path};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Select {
    path: Option<Path>,
}

impl Command for Select {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        model.select(&model.absolute_path(self.path.as_ref()))
    }
}
