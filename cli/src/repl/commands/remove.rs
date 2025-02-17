use crate::{
    repl::Repl,
    utils::{
        feedback::{error, success},
        path::Path,
    },
};
use clap::Args;
use colored::Colorize;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Remove {
    path: Path,
}

impl Command for Remove {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let mut path = model.absolute_path(Some(&self.path))?;

        let target_ident = path.pop().ok_or(error!("nothing to remove."))?;

        let parent = model.get_structure_from_path_mut(&path)?;

        parent.remove_child_boxed(&target_ident)?;

        let mut new_path = self.path.clone();

        while let Err(_) = model.absolute_path(Some(&new_path)) {
            new_path = new_path.join(&"..".into());
        }

        model.select(&model.absolute_path(Some(&new_path))?)?;

        println!("{}", success!("removed [{}].", target_ident.bold()));

        Ok(())
    }
}
