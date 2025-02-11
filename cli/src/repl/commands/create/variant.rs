use std::path::PathBuf;

use clap::Args;
use colored::Colorize;
use ir::structures::field::Numericity;

use crate::{repl::Repl, utils::numeric_value::NumericValue};

use super::CreateStructure;

#[derive(Debug, Clone, Args)]
pub struct Variant {
    #[arg(help = "Path to the variant")]
    path: PathBuf,
    #[arg(help = "The bit value this variant corresponds to")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    bits: Option<NumericValue>,

    #[arg(
        help = "Infer the variant bit value as one more than the variant with the highest bit value"
    )]
    #[arg(short, long)]
    next: bool,
}

impl CreateStructure for Variant {
    fn create(&self, model: &mut Repl) -> Result<(), String> {
        let mut segments = self.path.iter().rev();

        let Some(ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase()) else {
            Err(format!(
                "{}: variant identifier must be specified.",
                "error".red().bold()
            ))?
        };

        let Some(field_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase()) else {
            Err(format!(
                "{}: field identifier must be specified.",
                "error".red().bold()
            ))?
        };

        let Some(register_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase())
        else {
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

        let Some(register) = peripheral.registers.get_mut(&register_ident) else {
            Err(format!(
                "{}: register [{}/{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
            ))?
        };

        let Some(field) = register.fields.get_mut(&field_ident) else {
            Err(format!(
                "{}: field [{}/{}/{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
                field_ident.bold(),
            ))?
        };

        let Numericity::Enumerated { variants } = &mut field.numericity else {
            Err(format!(
                "{}: field [{}/{}/{}] is numeric and as such holds no variants.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
                field_ident.bold(),
            ))?
        };

        let None = variants.get(&ident) else {
            Err(format!(
                "{}: variant [{}/{}/{}/{}] already exists.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
                field_ident.bold(),
                ident.bold(),
            ))?
        };

        let bits = match (&self.bits, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}: next flag and bit value present, using specified bit value.",
                    "warning".yellow().bold()
                );
                **offset
            }
            (Some(offset), false) => **offset,
            (None, true) => variants
                .values()
                .max_by(|lhs, rhs| lhs.bits.cmp(&rhs.bits))
                .map_or(0, |last| last.bits + last.bits + 1), // next bit
            (None, false) => Err(format!(
                "{}: offset or next flag must be specified.",
                "error".red().bold()
            ))?,
        };

        variants.insert(
            ident.clone(),
            ir::structures::variant::Variant::empty(ident.clone(), bits),
        );

        println!(
            "{}: created [{}/{}/{}/{}].",
            "success".green().bold(),
            peripheral_ident.bold(),
            register_ident.bold(),
            field_ident.bold(),
            ident.bold()
        );

        Ok(())
    }
}
