use std::path::PathBuf;

use crate::repl::Repl;
use clap::Args;
use colored::Colorize;

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct Remove {
    path: PathBuf,
}

impl Command for Remove {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let mut segments = self.path.iter();

        let Some(peripheral_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase())
        else {
            Err(format!(
                "{}: structure path must be specified.",
                "error".red().bold()
            ))?
        };

        let Some(register_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase())
        else {
            return if model.hal.peripherals.remove(&peripheral_ident).is_none() {
                Err(format!(
                    "{}: peripheral [{}] does not exist.",
                    "error".red().bold(),
                    peripheral_ident.bold(),
                ))
            } else {
                println!(
                    "{}: removed [{}].",
                    "success".green().bold(),
                    peripheral_ident.bold(),
                );

                Ok(())
            };
        };

        let Some(peripheral) = model.hal.peripherals.get_mut(&peripheral_ident) else {
            Err(format!(
                "{}: peripheral [{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
            ))?
        };

        let Some(field_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase()) else {
            return if peripheral.registers.remove(&register_ident).is_none() {
                Err(format!(
                    "{}: register [{}/{}] does not exist.",
                    "error".red().bold(),
                    peripheral_ident.bold(),
                    register_ident.bold(),
                ))
            } else {
                println!(
                    "{}: removed [{}/{}].",
                    "success".green().bold(),
                    peripheral_ident.bold(),
                    register_ident.bold(),
                );

                Ok(())
            };
        };

        let Some(register) = peripheral.registers.get_mut(&register_ident) else {
            Err(format!(
                "{}: register [{}/{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
            ))?
        };

        let Some(variant_ident) = segments.next().map(|s| s.to_str().unwrap().to_uppercase())
        else {
            return if register.fields.remove(&field_ident).is_none() {
                Err(format!(
                    "{}: field [{}/{}/{}] does not exist.",
                    "error".red().bold(),
                    peripheral_ident.bold(),
                    register_ident.bold(),
                    field_ident.bold()
                ))
            } else {
                println!(
                    "{}: removed [{}/{}/{}].",
                    "success".green().bold(),
                    peripheral_ident.bold(),
                    register_ident.bold(),
                    field_ident.bold(),
                );
                Ok(())
            };
        };

        let Some(field) = register.fields.get_mut(&register_ident) else {
            Err(format!(
                "{}: field [{}/{}/{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
                field_ident.bold()
            ))?
        };

        let ir::structures::field::Numericity::Enumerated { variants } = &mut field.numericity
        else {
            Err(format!(
                "{}: attempted to remove variant from numeric field. only enumerated fields contain variants.",
                "error".red().bold()
            ))?
        };

        if variants.remove(&variant_ident).is_none() {
            Err(format!(
                "{}: variant [{}/{}/{}/{}] does not exist.",
                "error".red().bold(),
                peripheral_ident.bold(),
                register_ident.bold(),
                field_ident.bold(),
                variant_ident.bold()
            ))?
        }

        println!(
            "{}: removed [{}/{}/{}/{}].",
            "success".green().bold(),
            peripheral_ident.bold(),
            register_ident.bold(),
            field_ident.bold(),
            variant_ident.bold()
        );

        Ok(())
    }
}
