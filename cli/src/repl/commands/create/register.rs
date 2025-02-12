use std::path::PathBuf;

use clap::Args;
use colored::Colorize;

use crate::{
    repl::{commands::create::Structure, Repl},
    utils::{
        feedback::{error, success, warning},
        numeric_value::NumericValue,
        path::PathIter,
    },
};

use super::CreateStructure;

#[derive(Debug, Clone, Args)]
pub struct Register {
    #[arg(help = "Path to the register")]
    path: PathBuf,
    #[arg(help = "Register offset (bytes) within the peripheral")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    offset: Option<NumericValue>,

    #[arg(help = "Infer the register offset as the next byte after the last register")]
    #[arg(short, long)]
    next: bool,
}

impl CreateStructure for Register {
    fn create(&self, model: &mut Repl) -> Result<(), String> {
        let mut segments =
            PathIter::new(self.path.iter().map(|s| s.to_str().unwrap().to_uppercase()));

        let peripheral = model.hal.get_child_mut(&segments.next_segment()?)?;

        let ident = segments.next_segment()?;

        let offset = match (&self.offset, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}",
                    warning!("next flag and offset present, using specified offset.",)
                );
                **offset
            }
            (Some(offset), false) => **offset,
            (None, true) => peripheral
                .registers
                .values()
                .map(|register| register.offset)
                .max()
                .map_or(0, |last| last + 4), // registers are 32 bits wide which is 4 bytes
            (None, false) => Err(error!("offset or next flag must be specified.",))?,
        };

        peripheral.push_child(ir::structures::register::Register::empty(
            ident.clone(),
            offset,
        ))?;

        println!(
            "{}",
            success!("created [{}/{}].", peripheral.ident.bold(), ident.bold())
        );

        Ok(())
    }
}
