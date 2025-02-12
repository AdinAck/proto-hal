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
        let mut segments =
            PathIter::new(self.path.iter().map(|s| s.to_str().unwrap().to_uppercase()));

        let peripheral = model.hal.get_child_mut(&segments.next_segment()?)?;
        let register = peripheral.get_child_mut(&segments.next_segment()?)?;
        let field = register.get_child_mut(&segments.next_segment()?)?;

        let ident = segments.next_segment()?;

        let variants = field.children_mut()?;

        let bits = match (&self.bits, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}",
                    warning!("next flag and bit value present, using specified bit value."),
                );
                **offset
            }
            (Some(offset), false) => **offset,
            (None, true) => variants
                .values()
                .max_by(|lhs, rhs| lhs.bits.cmp(&rhs.bits))
                .map_or(0, |last| last.bits + last.bits + 1), // next bit
            (None, false) => Err(error!("offset or next flag must be specified."))?,
        };

        variants.insert(
            ident.clone(),
            ir::structures::variant::Variant::empty(ident.clone(), bits),
        );

        println!("{}", success!("created [{}].", ident.bold()));

        Ok(())
    }
}
