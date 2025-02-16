use crate::{repl::Repl, structures::DynStructure, utils::path::Path};
use clap::Args;
use prettytable::{row, Table};

use super::Command;

#[derive(Debug, Clone, Args)]
pub struct List {
    path: Option<Path>,
}

impl Command for List {
    fn execute(&self, model: &mut Repl) -> Result<(), String> {
        let mut structure: &dyn DynStructure = model.hal;

        if let Some(path) = &self.path {
            let mut segments = path.iter().map(|segment| segment.to_lowercase());

            loop {
                let ident = segments.next().unwrap();

                structure = structure.get_child_dyn(&ident)?;
            }
        }

        let mut peripherals = model.hal.peripherals.values().collect::<Vec<_>>();

        peripherals.sort_by(|lhs, rhs| lhs.base_addr.cmp(&rhs.base_addr));

        let mut table = Table::new();

        table.add_row(row![bu => "Address", "Identifier"]);

        for peripheral in peripherals {
            let addr = peripheral.base_addr;

            table.add_row(row![b -> format!("0x{addr:08x}"), FBb -> peripheral.ident]);
        }

        table.printstd();

        Ok(())
    }
}
