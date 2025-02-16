use crate::{
    repl::Repl,
    structures::DynStructure,
    utils::{feedback::success, path::Path},
};
use clap::Args;
use colored::Colorize;
use ir::structures::hal::Hal;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Remove {
    path: Path,
}

impl Remove {
    fn remove(&self, hal: &mut Hal) -> Result<String, String> {
        let mut segments = self
            .path
            .iter()
            .map(|segment| segment.to_lowercase())
            .peekable();

        let mut structure: &mut dyn DynStructure = hal;

        loop {
            let ident = segments.next().unwrap();

            if segments.peek().is_none() {
                break structure
                    .remove_child_boxed(&ident)
                    .map(|_| success!("removed [{}].", ident.bold()));
            }
            structure = structure.get_child_dyn_mut(&ident)?;
        }
    }
}

impl Command for Remove {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        self.remove(model.hal).map(|msg| println!("{msg}"))
    }
}
