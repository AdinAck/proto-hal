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

    // array
    #[arg(
        help = "Generate a sequence of variants, beginning with the specified index. <PATH> must contain exacly one \"X\" for replacement"
    )]
    #[arg(long)]
    start: Option<usize>,
    #[arg(
        help = "Generate a sequence of variants, ending with the specified index. <PATH> must contain exacly one \"X\" for replacement"
    )]
    #[arg(long)]
    end: Option<usize>,
    #[arg(
        help = "Generate a sequence of variants, incrementing the index by this step. <PATH> must contain exacly one \"X\" for replacement"
    )]
    #[arg(long)]
    step: Option<usize>,
}

impl Command for CreateVariant {
    fn execute(&self, model: &mut Repl) -> Result<(), Diagnostic> {
        let path = model.absolute_path(Some(&self.path));
        let mut segments = PathIter::new(path.iter());

        let peripheral = model.hal.get_child_mut(&segments.next_segment()?)?;
        let register = peripheral.get_child_mut(&segments.next_segment()?)?;
        let field = register.get_child_mut(&segments.next_segment()?)?;

        let ident = segments.next_segment()?;

        let variants = field.children()?;

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
                .map_or(0, |last| last.bits + 1), // next bit
            (None, false) => Err(Diagnostic::error(
                "offset or next flag must be specified.".to_owned(),
            ))?,
        };

        let range = self.start.unwrap_or(0)..=self.end.unwrap_or((1 << field.width) - 1);
        let step = self.step.unwrap_or(1);

        let replacement = self.start.is_some() || self.end.is_some() || self.step.is_some();

        if replacement && !ident.contains("X") {
            Err(Diagnostic::error(
                "variant path must contain exactly one \"X\" for replacement".to_string(),
            ))?
        }

        for (i, x) in range.step_by(step).enumerate() {
            let replaced_ident = ident.replace("X", x.to_string().as_str());

            field.push_child(ir::structures::variant::Variant::empty(
                replaced_ident.clone(),
                bits + i as u32,
            ))?;

            println!(
                "{}",
                success!(
                    "created [{}] in [{}] with bit value {}.",
                    replaced_ident.bold(),
                    field.ident.bold(),
                    (bits + i as u32).to_string().bold()
                )
            );
        }

        Ok(())
    }
}
