use crate::{repl::Repl, utils::path::Path};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Select {
    path: Option<Path>,
}

impl Command for Select {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        if let Some(path) = &self.path {
            model.select(path)?;
        } else {
            model.select_path = Path::empty();
        }

        Ok(())
    }
}
