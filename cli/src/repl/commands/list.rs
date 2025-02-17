use crate::{repl::Repl, utils::path::Path};
use clap::Args;

use super::{tree::Tree, Command};

#[derive(Debug, Clone, Args)]
pub struct List {
    path: Option<Path>,
}

impl Command for List {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        Tree::new(self.path.clone(), Some(1)).execute(model)
    }
}
