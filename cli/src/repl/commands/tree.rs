use crate::{repl::Repl, utils::path::Path};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Tree {
    path: Option<Path>,
    #[arg(short, long, help = "Maximum depth to traverse.")]
    depth: Option<usize>,
}

impl Command for Tree {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let structure = model.get_structure_from_path(&model.absolute_path(self.path.as_ref())?)?;

        println!("{}", structure.tree(self.depth));

        Ok(())
    }
}
