use clap::Args;
use colored::Colorize;

use crate::{
    repl::{commands::create::Structure, Repl},
    utils::numeric_value::NumericValue,
};

use super::CreateStructure;

#[derive(Debug, Clone, Args)]
pub struct Peripheral {
    #[arg(help = "The peripheral identifier")]
    ident: String,
    #[arg(help = "The base address of the peripheral (base 10 or base 16)")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    base_addr: NumericValue,
}

impl CreateStructure for Peripheral {
    fn create(&self, model: &mut Repl) -> Result<(), String> {
        let ident = self.ident.to_uppercase();

        model
            .hal
            .push_child(ir::structures::peripheral::Peripheral::empty(
                ident.clone(),
                *self.base_addr,
            ))?;

        println!("{}: created [{}].", "success".green().bold(), ident.bold());

        Ok(())
    }
}
