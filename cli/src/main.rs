use clap::{Parser, Subcommand};
use quote::ToTokens;
use repl::{commands::Command, Repl};
use std::{fs, path::PathBuf};

use ir::structures::hal::Hal;

mod repl;
mod structures;
mod utils;

#[derive(Subcommand)]
enum Commands {
    Init {
        path: PathBuf,
    },
    Open {
        file_path: PathBuf,
        #[arg(name = "script", long, help = "Load and run a script")]
        script_path: Option<PathBuf>,
    },
    Check {
        file_path: PathBuf,
    },
    Generate {
        input_file_path: PathBuf,
        output_file_path: PathBuf,
    },
}

#[derive(Parser)]
#[command(name = "proto-hal-cli")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => fs::write(
            path.with_extension("toml"),
            toml::to_string_pretty(&Hal::empty()).unwrap(),
        )
        .unwrap(),
        Commands::Open {
            file_path,
            script_path,
        } => {
            let mut hal = toml::from_str(&fs::read_to_string(&file_path).unwrap()).unwrap();
            let mut repl = Repl::new(&mut hal, &file_path);

            if let Some(script_path) = script_path {
                repl::commands::script::Script::new(script_path)
                    .execute(&mut repl)
                    .unwrap();
            }

            repl.run().unwrap();
        }
        Commands::Generate {
            input_file_path,
            output_file_path,
        } => {
            let hal: Hal = toml::from_str(&fs::read_to_string(&input_file_path).unwrap()).unwrap();

            fs::write(output_file_path, hal.to_token_stream().to_string()).unwrap();
        }
        _ => todo!(),
    }
}
