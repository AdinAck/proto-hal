use std::path::PathBuf;

use crate::repl::Repl;
use clap::Args;
use colored::Colorize;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Info {
    path: Option<PathBuf>,
}

impl Command for Info {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let Some(path) = &self.path else {
            println!("nothing to display.");

            return Ok(());
        };

        Ok(())
    }
}
