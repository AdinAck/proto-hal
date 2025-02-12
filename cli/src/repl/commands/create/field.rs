use std::{collections::HashMap, path::PathBuf};

use clap::{Args, ValueEnum};
use colored::Colorize;

use crate::{
    repl::{commands::create::FromParent, Repl},
    utils::{
        feedback::{error, success, warning},
        numeric_value::NumericValue,
        path::PathIter,
    },
};

use super::CreateStructure;

#[derive(Debug, Clone, Default, ValueEnum)]
enum Numericity {
    Numeric,
    #[default]
    Enumerated,
}

#[derive(Debug, Clone, Args)]
pub struct Field {
    #[arg(help = "Path to the field")]
    path: PathBuf,
    #[arg(help = "The width (bits) of the field")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    width: NumericValue,
    #[arg(
        help = "The numericity of the field. A field's state either represents a purely numeric value, or enumerated variants."
    )]
    #[arg(default_value = "enumerated")]
    numericity: Numericity,
    #[arg(help = "Field offset (bits) within the register")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    offset: Option<NumericValue>,

    #[arg(help = "Infer the field offset as the next bit after the last field")]
    #[arg(short, long)]
    next: bool,
}

impl CreateStructure for Field {
    fn create(&self, model: &mut Repl) -> Result<(), String> {
        let mut segments =
            PathIter::new(self.path.iter().map(|s| s.to_str().unwrap().to_uppercase()));

        let peripheral = ir::structures::peripheral::Peripheral::from_parent_mut(
            model.hal,
            &segments.next_segment()?,
        )?;

        let register = ir::structures::register::Register::from_parent_mut(
            peripheral,
            &segments.next_segment()?,
        )?;

        let ident = segments.next_segment()?;

        let None = register.fields.get(&ident) else {
            Err(error!("field [{}] already exists.", ident.bold(),))?
        };

        let offset = match (&self.offset, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}",
                    warning!("next flag and offset present, using specified offset.",)
                );
                **offset
            }
            (Some(offset), false) => **offset,
            (None, true) => register
                .fields
                .values()
                .max_by(|lhs, rhs| lhs.offset.cmp(&rhs.offset))
                .map_or(0, |last| last.offset as u32 + last.width as u32 + 1), // next bit
            (None, false) => Err(error!("offset or next flag must be specified.",))?,
        };

        register.fields.insert(
            ident.clone(),
            ir::structures::field::Field::empty(
                ident.clone(),
                offset.try_into().map_err(|e| error!("{e}"))?,
                (*self.width).try_into().map_err(|e| error!("{e}"))?,
                match self.numericity {
                    Numericity::Numeric => ir::structures::field::Numericity::Numeric,
                    Numericity::Enumerated => ir::structures::field::Numericity::Enumerated {
                        variants: HashMap::new(),
                    },
                },
            ),
        );

        println!("{}", success!("created [{}].", ident.bold()));

        Ok(())
    }
}
