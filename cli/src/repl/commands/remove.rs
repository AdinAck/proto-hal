use std::path::PathBuf;

use crate::{
    repl::Repl,
    utils::{
        feedback::{error, success},
        path::PathIter,
    },
};
use clap::Args;
use colored::Colorize;
use ir::structures::{field::Numericity, hal::Hal};

use super::{create::Structure, Command};

#[derive(Debug, Clone, Args)]
pub struct Remove {
    path: PathBuf,
}

impl Remove {
    fn remove(&self, hal: &mut Hal) -> Result<String, String> {
        let mut segments =
            PathIter::new(self.path.iter().map(|s| s.to_str().unwrap().to_uppercase()));

        let peripheral_ident = &segments.next_segment().unwrap(); // there is necessarily at least one segment

        let peripheral =
            ir::structures::peripheral::Peripheral::from_parent_mut(hal, peripheral_ident)?;

        let Ok(register_ident) = &segments.next_segment() else {
            hal.peripherals.remove(peripheral_ident);

            return Ok(success!("removed [{}].", peripheral_ident.bold()));
        };

        let register =
            ir::structures::register::Register::from_parent_mut(peripheral, register_ident)?;

        let Ok(field_ident) = &segments.next_segment() else {
            peripheral.registers.remove(register_ident);

            return Ok(success!("removed [{}].", register_ident.bold()));
        };

        let field = ir::structures::field::Field::from_parent_mut(register, field_ident)?;

        let Ok(variant_ident) = &segments.next_segment() else {
            register.fields.remove(register_ident);

            return Ok(success!("removed [{}].", field_ident.bold()));
        };

        let Numericity::Enumerated { variants } = &mut field.numericity else {
            Err(error!(
                "field [{}] is numeric and as such holds no variants.",
                field_ident
            ))?
        };

        variants.remove(variant_ident);

        Ok(success!("removed [{}].", variant_ident))
    }
}

impl Command for Remove {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        self.remove(model.hal).map(|msg| println!("{msg}"))
    }
}
