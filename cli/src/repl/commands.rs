use clap::Subcommand;
use commit::Commit;
use create::Create;
use diff::Diff;
use discard::Discard;
use enum_dispatch::enum_dispatch;
use exit::Exit;
use info::Info;
use list::List;
use remove::Remove;

use crate::repl::Repl;

pub mod commit;
pub mod create;
pub mod diff;
pub mod discard;
pub mod exit;
pub mod info;
pub mod list;
pub mod remove;

#[enum_dispatch]
pub trait Command {
    fn execute(&self, repl: &mut Repl) -> Result<(), String>;
}

#[enum_dispatch(Command)]
#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(subcommand)]
    Create,
    #[command(about = "Remove the selected structure")]
    Remove,
    #[command(about = "Commit pending changes to the open buffer")]
    Commit,
    #[command(about = "Discard pending changes")]
    Discard,
    #[command(alias = "l")]
    #[command(
        about = "List currently scoped structures or specified nested structures [alias: l]"
    )]
    List,
    #[command(
        about = "Display info about currently scoped structure or specified nested structure"
    )]
    Info,
    #[command(about = "View a diff view of pending changes")]
    Diff,
    #[command(about = "Exit the interactive REPL")]
    Exit,
}
