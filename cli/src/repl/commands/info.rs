use crate::{repl::Repl, structures::DynStructure, utils::path::Path};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Info {
    path: Option<Path>,
}

impl Command for Info {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let path = model
            .select_path
            .join(self.path.as_ref().unwrap_or(&Path::empty()));

        let segments = path.iter().map(|segment| segment.to_lowercase()).peekable();

        let mut structure: &mut dyn DynStructure = model.hal;

        for segment in segments {
            structure = structure.get_child_dyn_mut(&segment)?;
        }

        println!("{}", structure.info());

        Ok(())
    }
}
