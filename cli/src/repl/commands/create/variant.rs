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
pub struct CreateVariant {
    #[arg(help = "Path to the variant")]
    path: Path,
    #[arg(help = "The bit value this variant corresponds to")]
    #[arg(value_parser = clap::value_parser!(NumericValue))]
    bits: Option<NumericValue>,

    #[arg(
        help = "Infer the variant bit value as one more than the variant with the highest bit value"
    )]
    #[arg(short, long)]
    next: bool,
}

impl Command for CreateVariant {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        let path = model.absolute_path(Some(&self.path));
        let mut segments = PathIter::new(path.iter());

        let peripheral = model.hal.get_child_mut(&segments.next_segment()?)?;
        let register = peripheral.get_child_mut(&segments.next_segment()?)?;
        let field = register.get_child_mut(&segments.next_segment()?)?;

        let ident = segments.next_segment()?;

        let variants = field.children_mut()?;

        let bits = match (&self.bits, self.next) {
            (Some(offset), true) => {
                eprintln!(
                    "{}",
                    Diagnostic::warning(
                        "next flag and bit value present, using specified bit value.".to_owned()
                    ),
                );
                **offset
            }
            (Some(offset), false) => **offset,
            (None, true) => variants
                .values()
                .max_by(|lhs, rhs| lhs.bits.cmp(&rhs.bits))
                .map_or(0, |last| last.bits + last.bits + 1), // next bit
            (None, false) => Err(Diagnostic::error(
                "offset or next flag must be specified.".to_owned(),
            ))?,
        };

        variants.insert(
            ident.to_owned(),
            ir::structures::variant::Variant::empty(ident.to_owned(), bits),
        );

        println!(
            "{}",
            success!(
                "created [{}] in [{}] with bit value {}.",
                ident.bold(),
                field.ident.bold(),
                bits.to_string().bold()
            )
        );

        Ok(())
    }
}
