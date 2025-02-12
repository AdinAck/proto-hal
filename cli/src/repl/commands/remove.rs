use std::path::PathBuf;

use crate::{
    repl::{commands::create::DynStructure, Repl},
    utils::feedback::success,
};
use clap::Args;
use colored::Colorize;
use ir::structures::hal::Hal;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Remove {
    path: PathBuf,
}

impl Remove {
    fn remove(&self, hal: &mut Hal) -> Result<String, String> {
        let mut segments = self
            .path
            .iter()
            .map(|s| s.to_str().unwrap().to_uppercase())
            .peekable();

        let mut structure: Box<&mut dyn DynStructure> = Box::new(hal);

        loop {
            let ident = segments.next().unwrap();

            if segments.peek().is_none() {
                break structure
                    .remove_child_boxed(&ident)
                    .map(|_| success!("removed [{}].", ident.bold()));
            }
            structure = structure.get_child_boxed_mut(&ident)?;
        }
    }
}

impl Command for Remove {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        self.remove(model.hal).map(|msg| println!("{msg}"))
    }
}
