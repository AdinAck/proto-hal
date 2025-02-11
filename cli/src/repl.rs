use clap::Parser;
use colored::Colorize;
use commands::{Command, Commands};
use ir::structures::hal::Hal;
use rustyline::{config::Configurer, error::ReadlineError, DefaultEditor};
use std::{fs, path::PathBuf};

pub mod commands;

const HOME_PATH: &str = env!("HOME");
const LOCAL_STORAGE_PATH: &str = ".proto-hal";
const HISTORY_PATH: &str = "history";

pub struct Repl<'a> {
    hal: &'a mut Hal,
    old_hal: Hal,
    file: &'a PathBuf,

    quit: bool,
}

impl<'a> Repl<'a> {
    pub fn new(hal: &'a mut Hal, file: &'a PathBuf) -> Self {
        let old_hal = hal.clone();
        Self {
            hal,
            old_hal,
            file,
            quit: false,
        }
    }

    fn respond(&mut self, cmd: &str) -> Result<(), String> {
        let args = shlex::split(cmd).ok_or("error: Invalid quoting")?;
        let cli = Cli::try_parse_from(args).map_err(|e| e.to_string())?;
        cli.command.execute(self)?;
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
        if !self.changes_pending() {
            format!("{}> ", "proto-hal".green().bold())
        } else {
            format!("{} ({})> ", "proto-hal".green().bold(), "?".yellow().bold())
        }
    }

    pub fn exit(&mut self) -> Result<(), String> {
        self.quit = if self.changes_pending() {
            let decision = Self::confirmation_dialog()?;

            if decision {
                true
            } else {
                false
            }
        } else {
            true
        };

        Ok(())
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

                // attemp the execute the command
                if let Err(e) = self.respond(&cmd) {
                    // if the command errors, report the error
                    eprintln!("{e}");
                }
            }

            // if the repl has decided to exit, do so
            if self.quit {
                break;
            }
        }

        if !fs::exists(&PathBuf::from(HOME_PATH).join(LOCAL_STORAGE_PATH)).unwrap() {
            fs::create_dir_all(&PathBuf::from(HOME_PATH).join(LOCAL_STORAGE_PATH)).unwrap();
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
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
