use clap::Args;
use colored::Colorize;
use ir::utils::diagnostic::Diagnostic;

use crate::{
    repl::{commands::Command, Repl},
    structures::Structure,
    utils::{
        feedback::success,
        numeric_value::NumericValue,
        path::{Path, PathIter},
    },
};

#[derive(Debug, Clone, Args)]
pub struct CreateRegister {
    #[arg(help = "Path to the register")]
    path: Path,
    #[arg(help = "Register offset (bytes) within the peripheral")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    offset: Option<NumericValue>,

    #[arg(help = "Infer the register offset as the next byte after the last register")]
    #[arg(short, long)]
    next: bool,
}

impl Command for CreateRegister {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        let path = model.absolute_path(Some(&self.path));
        let mut segments = PathIter::new(path.iter());

        let peripheral = model.hal.get_child_mut(&segments.next_segment()?)?;

        let ident = segments.next_segment()?;

        let offset = match (&self.offset, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}",
                    Diagnostic::warning(
                        "next flag and offset present, using specified offset.".to_owned()
                    )
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
            (None, false) => Err(Diagnostic::error(
                "offset or next flag must be specified.".to_owned(),
            ))?,
        };

        peripheral.push_child(ir::structures::register::Register::empty(
            ident.to_owned(),
            offset,
        ))?;

        println!(
            "{}",
            success!(
                "created [{}] in [{}] at offset {}.",
                ident.bold(),
                peripheral.ident.bold(),
                offset.to_string().bold(),
            )
        );

        Ok(())
    }
}
