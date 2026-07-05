mod ast;
mod lexer;
mod parser;

use ariadne::{Color, Config, IndexType, Label, Report, ReportKind, Source};
use chumsky::{input::Input as _, prelude::*};

use crate::ast::Span;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    let owned;
    let (filename, src): (&str, &str) = match args.get(1) {
        Some(path) => {
            owned = match std::fs::read_to_string(path) {
                Ok(src) => src,
                Err(e) => {
                    eprintln!("failed to read `{path}`: {e}");
                    std::process::exit(1);
                }
            };

            (path.as_str(), owned.as_str())
        }
        None => ("example.phm", include_str!("example.phm")),
    };

    let (tokens, errors) = lexer::lexer().parse(src).into_output_errors();

    let mut failed = !errors.is_empty();
    report("lexical error", filename, src, errors);

    let Some(tokens) = tokens else {
        std::process::exit(1);
    };

    let eoi = Span::from(src.len()..src.len());
    let input = tokens.as_slice().map(eoi, |(token, span)| (token, span));

    let (file, errors) = parser::file_parser().parse(input).into_output_errors();

    failed |= !errors.is_empty();
    report("syntax error", filename, src, errors);

    let Some(_file) = file else {
        std::process::exit(1);
    };

    // println!("{_file:#?}");

    if failed {
        std::process::exit(1);
    }
}

fn report<'err, T>(kind: &'static str, filename: &str, src: &str, errors: Vec<Rich<'err, T, Span>>)
where
    T: std::fmt::Display,
{
    for error in errors {
        Report::build(
            ReportKind::Custom(kind, Color::Red),
            (filename, error.span().into_range()),
        )
            // chumsky spans are byte offsets; ariadne defaults to char indices
            .with_config(Config::default().with_index_type(IndexType::Byte))
            .with_message(error.to_string())
            .with_label(
                Label::new((filename, error.span().into_range()))
                    .with_message(error.reason().to_string())
                    .with_color(Color::Red),
            )
            .with_labels(error.contexts().map(|(label, span)| {
                Label::new((filename, span.into_range()))
                    .with_message(format!("while parsing this {label}"))
                    .with_color(Color::Yellow)
            }))
            .finish()
            .print((filename, Source::from(src)))
            .unwrap();
    }
}
