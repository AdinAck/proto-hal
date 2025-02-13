use std::path::PathBuf;

use crate::{repl::Repl, structures::DynStructure};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Info {
    path: Option<PathBuf>,
}

impl Command for Info {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let Some(path) = &self.path else {
            println!("{}", model.hal.info());

            return Ok(());
        };

        let segments = path
            .iter()
            .map(|s| s.to_str().unwrap().to_lowercase())
            .peekable();

        let mut structure: Box<&mut dyn DynStructure> = Box::new(model.hal);

        for segment in segments {
            structure = structure.get_child_boxed_mut(&segment)?;
        }

        println!("{}", structure.info());

        Ok(())
    }
}
