use clap::Parser;
use colored::Colorize;
use commands::{
    Command as _, CommandsAtField, CommandsAtHal, CommandsAtPeripheral, CommandsAtRegister,
    CommandsAtVariant,
};
use ir::{structures::hal::Hal, utils::diagnostic::Diagnostic};
use rustyline::{config::Configurer, error::ReadlineError, DefaultEditor};
use std::{fs, path::PathBuf};

use crate::{
    structures::{DynStructure, StructureKind},
    utils::{feedback::error, path::Path},
};

pub mod commands;

const HOME_PATH: &str = env!("HOME");
const LOCAL_STORAGE_PATH: &str = ".proto-hal";
const HISTORY_PATH: &str = "history";

pub struct Repl<'a> {
    hal: &'a mut Hal,
    old_hal: Hal,
    file: &'a PathBuf,

    select_path: Path,
    structure: StructureKind,
    diagnostics: Vec<Diagnostic>,

    quit: bool,
}

impl<'a> Repl<'a> {
    pub fn new(hal: &'a mut Hal, file: &'a PathBuf) -> Self {
        let old_hal = hal.clone();
        Self {
            hal,
            old_hal,
            file,
            select_path: Path::empty(),
            structure: StructureKind::Hal,
            diagnostics: Vec::new(),
            quit: false,
        }
    }

    fn execute(&mut self, cmd: &str) -> Result<(), String> {
        let args = shlex::split(cmd).ok_or("error: Invalid quoting")?;

        match &self.structure {
            StructureKind::Hal => {
                let cli = CliAtHal::try_parse_from(args).map_err(|e| e.to_string())?;
                cli.command.execute(self)?;
            }
            StructureKind::Peripheral => {
                let cli = CliAtPeripheral::try_parse_from(args).map_err(|e| e.to_string())?;
                cli.command.execute(self)?;
            }
            StructureKind::Register => {
                let cli = CliAtRegister::try_parse_from(args).map_err(|e| e.to_string())?;
                cli.command.execute(self)?;
            }
            StructureKind::Field => {
                let cli = CliAtField::try_parse_from(args).map_err(|e| e.to_string())?;
                cli.command.execute(self)?;
            }
            StructureKind::Variant => {
                let cli = CliAtVariant::try_parse_from(args).map_err(|e| e.to_string())?;
                cli.command.execute(self)?;
            }
        }

        Ok(())
    }

    fn changes_pending(&self) -> bool {
        !self.old_hal.eq(self.hal)
    }

    fn confirmation_dialog() -> Result<bool, String> {
        let mut rl = DefaultEditor::new().unwrap();

        Ok(loop {
            let decision = match rl.readline("there are pending changes, are you sure? [y/n] ") {
                Ok(decision) => decision,
                Err(ReadlineError::Interrupted) => continue,
                Err(e) => Err(e.to_string())?,
            };

            match decision.to_lowercase().as_str() {
                "y" => break true,
                "n" => break false,
                _ => continue,
            }
        })
    }

    fn prompt(&self) -> String {
        let mut components = vec!["proto-hal".green().bold().to_string()];

        if self.select_path.to_string() != "" {
            components.push(
                self.select_path
                    .iter()
                    .map(|segment| segment.bold().to_string())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }

        if self.changes_pending() {
            components.push(format!("({})", "?".yellow().bold()));
        }

        let mut prompt = components.join(" ");
        prompt.push_str("> ");

        prompt
    }

    pub fn exit(&mut self) -> Result<(), String> {
        self.quit = if self.changes_pending() {
            Self::confirmation_dialog()?
        } else {
            true
        };

        Ok(())
    }

    pub fn absolute_path(&self, path: Option<&Path>) -> Path {
        let Some(path) = path else {
            return self.select_path.clone();
        };

        let mut new_path = self.select_path.clone();
        for segment in path.iter() {
            new_path = match segment {
                ".." => {
                    new_path.pop();
                    new_path
                }
                "/" => Path::empty(),
                _ => new_path.join(&segment.into()),
            };
        }

        new_path
    }

    pub fn validate_path(&self, path: &Path) -> Result<(), String> {
        self.get_structure_from_path(path)?;

        Ok(())
    }

    pub fn select(&mut self, path: &Path) -> Result<(), String> {
        let mut kind = StructureKind::Hal;
        for segment in path.iter() {
            kind = kind
                .child()
                .ok_or(error!("[{}] has no children.", segment.bold()))?;
        }

        self.validate_path(path)?;

        self.structure = kind;
        self.select_path = path.clone();

        Ok(())
    }

    pub fn get_structure_from_path(&self, path: &Path) -> Result<&dyn DynStructure, String> {
        let mut structure: &dyn DynStructure = self.hal;

        for segment in path.iter() {
            structure = structure.get_child_dyn(segment)?;
        }

        Ok(structure)
    }

    pub fn get_structure_from_path_mut(
        &mut self,
        path: &Path,
    ) -> Result<&mut dyn DynStructure, String> {
        let mut structure: &mut dyn DynStructure = self.hal;

        for segment in path.iter() {
            structure = structure.get_child_dyn_mut(segment)?;
        }

        Ok(structure)
    }

    pub fn run(&mut self) -> Result<(), String> {
        let mut rl = DefaultEditor::new().unwrap();
        rl.set_auto_add_history(true);
        rl.load_history(
            &PathBuf::from(HOME_PATH)
                .join(LOCAL_STORAGE_PATH)
                .join(HISTORY_PATH),
        )
        .ok();

        loop {
            // attempt to ingest a command
            if let Some(cmd) = match rl.readline(&self.prompt()) {
                Ok(line) => Some(line),
                // if the user attempts to exit
                Err(ReadlineError::Interrupted) => {
                    // ask the repl to exit
                    self.exit()?;
                    None
                }
                // if a readline error occurs
                Err(e) => {
                    // report it and continue the repl
                    eprintln!("{e}");
                    continue;
                }
            } {
                // the command is successfully ingested

                if cmd.is_empty() {
                    // if the command is empty, ignore
                    // it and continue the repl
                    continue;
                }

                let stored_hal = self.hal.clone();

                // attemp the execute the command
                if let Err(e) = self.execute(&cmd) {
                    // if the command errors, report the error
                    eprintln!("{e}");
                }

                self.diagnostics = self.hal.validate();

                if !self.diagnostics.is_empty() {
                    *self.hal = stored_hal;
                    eprintln!("{}", Diagnostic::report(&self.diagnostics));
                }
            }

            // if the repl has decided to exit, do so
            if self.quit {
                break;
            }
        }

        if !fs::exists(PathBuf::from(HOME_PATH).join(LOCAL_STORAGE_PATH)).unwrap() {
            fs::create_dir_all(PathBuf::from(HOME_PATH).join(LOCAL_STORAGE_PATH)).unwrap();
        }

        rl.save_history(
            &PathBuf::from(HOME_PATH)
                .join(LOCAL_STORAGE_PATH)
                .join(HISTORY_PATH),
        )
        .unwrap();

        Ok(())
    }
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct CliAtHal {
    #[command(subcommand)]
    command: CommandsAtHal,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct CliAtPeripheral {
    #[command(subcommand)]
    command: CommandsAtPeripheral,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct CliAtRegister {
    #[command(subcommand)]
    command: CommandsAtRegister,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct CliAtField {
    #[command(subcommand)]
    command: CommandsAtField,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct CliAtVariant {
    #[command(subcommand)]
    command: CommandsAtVariant,
}
