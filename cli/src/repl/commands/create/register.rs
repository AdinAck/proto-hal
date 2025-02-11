use std::path::PathBuf;

use clap::Args;
use colored::Colorize;

use crate::{repl::Repl, utils::numeric_value::NumericValue};

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
        let mut segments = self.path.iter().rev();

        let Some(ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase()) else {
            Err(format!(
                "{}: register identifier must be specified.",
                "error".red().bold()
            ))?
        };

        let Some(peripheral_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase())
        else {
            Err(format!(
                "{}: peripheral identifier must be specified.",
                "error".red().bold()
            ))?
        };

        let Some(peripheral) = model.hal.peripherals.get_mut(&peripheral_ident) else {
            Err(format!(
                "{}: peripheral [{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
            ))?
        };

        let None = peripheral.registers.get(&ident) else {
            Err(format!(
                "{}: register [{}/{}] already exists.",
                "error".red().bold(),
                peripheral_ident.bold(),
                ident.bold(),
            ))?
        };

        let offset = match (&self.offset, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}: next flag and offset present, using specified offset.",
                    "warning".yellow().bold()
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
            (None, false) => Err(format!(
                "{}: offset or next flag must be specified.",
                "error".red().bold()
            ))?,
        };

        peripheral.registers.insert(
            ident.clone(),
            ir::structures::register::Register::empty(ident.clone(), offset),
        );

        println!(
            "{}: created [{}/{}].",
            "success".green().bold(),
            peripheral_ident.bold(),
            ident.bold()
        );

        Ok(())
    }
}
