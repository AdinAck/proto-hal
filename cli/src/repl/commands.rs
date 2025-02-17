use clap::Subcommand;
use commit::Commit;
use create::{
    field::CreateField, peripheral::CreatePeripheral, register::CreateRegister,
    variant::CreateVariant,
};
use diff::Diff;
use discard::Discard;
use enum_dispatch::enum_dispatch;
use exit::Exit;
use info::Info;
use list::List;
use remove::Remove;
use script::Script;
use select::Select;
use tree::Tree;

use crate::repl::Repl;

pub mod commit;
pub mod create;
pub mod diff;
pub mod discard;
pub mod exit;
pub mod info;
pub mod list;
pub mod remove;
pub mod script;
pub mod select;
pub mod tree;

#[enum_dispatch]
pub trait Command {
    fn execute(&self, repl: &mut Repl) -> Result<(), String>;
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum CommandsAtHal {
    #[command(name = "create")]
    CreatePeripheral,

    #[command(flatten)]
    GlobalCommands,
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum CommandsAtPeripheral {
    #[command(name = "create")]
    CreateRegister,

    #[command(flatten)]
    GlobalCommands,
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum CommandsAtRegister {
    #[command(name = "create")]
    CreateField,

    #[command(flatten)]
    GlobalCommands,
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum CommandsAtField {
    #[command(name = "create")]
    CreateVariant,

    #[command(flatten)]
    GlobalCommands,
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum CommandsAtVariant {
    #[command(flatten)]
    GlobalCommands,
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum GlobalCommands {
    #[command(about = "Remove the selected structure")]
    Remove,
    #[command(about = "Commit pending changes to the open buffer")]
    Commit,
    #[command(about = "Discard pending changes")]
    Discard,
    #[command(visible_alias = "t")]
    #[command(
        about = "Display a tree view of currently scoped structures or specified nested structures"
    )]
    Tree,
    #[command(visible_alias = "l")]
    #[command(about = "List currently scoped structures or specified nested structures")]
    List,
    #[command(
        about = "Display info about currently scoped structure or specified nested structure"
    )]
    Info,
    #[command(about = "View a diff view of pending changes")]
    Diff,
    #[command(visible_alias = "cd")]
    #[command(about = "Select a structure")]
    Select,
    #[command(about = "Load and run a script")]
    Script,
    #[command(about = "Exit the interactive REPL")]
    Exit,
}
