use clap::Args;
use colored::Colorize;

use crate::{
    repl::{commands::Command, Repl},
    structures::Structure,
    utils::{feedback::success, numeric_value::NumericValue},
};

#[derive(Debug, Clone, Args)]
pub struct CreatePeripheral {
    #[arg(help = "The peripheral identifier")]
    ident: String,
    #[arg(help = "The base address of the peripheral (base 10 or base 16)")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    base_addr: NumericValue,
}

impl Command for CreatePeripheral {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let ident = self.ident.to_lowercase();

        model
            .hal
            .push_child(ir::structures::peripheral::Peripheral::empty(
                ident.clone(),
                *self.base_addr,
            ))?;

        println!(
            "{}",
            success!(
                "created [{}] at {}.",
                ident.bold(),
                format!("0x{:08x}", *self.base_addr).bold()
            )
        );

        Ok(())
    }
}
