use std::path::PathBuf;

use crate::repl::Repl;
use clap::Args;
use prettytable::{row, Table};

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct List {
    path: Option<PathBuf>,
}

impl Command for List {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let mut peripherals = model.hal.peripherals.values().collect::<Vec<_>>();

        peripherals.sort_by(|lhs, rhs| lhs.base_addr.cmp(&rhs.base_addr));

        let mut table = Table::new();

        table.add_row(row![bu => "Address", "Identifier"]);

        for peripheral in peripherals {
            let addr = peripheral.base_addr;

            table.add_row(row![b -> format!("0x{addr:x}"), FBb -> peripheral.ident]);
        }

        table.printstd();

        Ok(())
    }
}
