use crate::repl::Repl;
use clap::Subcommand;
use enum_dispatch::enum_dispatch;
use field::Field;
use peripheral::Peripheral;
use register::Register;
use variant::Variant;

use super::Command;

pub mod field;
pub mod peripheral;
pub mod register;
pub mod variant;

#[enum_dispatch]
pub trait CreateStructure {
    fn create(&self, model: &mut Repl) -> Result<(), String>;
}

#[enum_dispatch(CreateStructure)]
#[derive(Debug, Clone, Subcommand)]
#[command(about = "Create a new structure")]
pub enum Create {
    #[command(alias = "p", about = "[alias: p]")]
    Peripheral,
    #[command(alias = "r", about = "[alias: r]")]
    Register,
    #[command(alias = "f", about = "[alias: f]")]
    Field,
    #[command(alias = "v", about = "[alias: v]")]
    Variant,
}

impl Command for Create {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        self.create(model)
    }
}
