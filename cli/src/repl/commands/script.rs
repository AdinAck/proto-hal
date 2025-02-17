use std::{fs, path::PathBuf};

use crate::{
    repl::Repl,
    utils::feedback::{error, success},
};
use clap::Args;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Script {
    file_path: PathBuf,
}

impl Script {
    pub const fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }
}

impl Command for Script {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let script = fs::read_to_string(&self.file_path).map_err(|e| error!("fs error: {e}."))?;

        for cmd in script.lines() {
            model.execute(cmd)?;
        }

        println!("{}", success!("script finished."));

        Ok(())
    }
}
