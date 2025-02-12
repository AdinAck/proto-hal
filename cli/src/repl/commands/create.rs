use std::{collections::HashMap, path::PathBuf};

use crate::{repl::Repl, utils::feedback::error};
use clap::Subcommand;
use colored::Colorize;
use enum_dispatch::enum_dispatch;
use field::Field;
use ir::structures::{field::Numericity, hal::Hal};
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

pub trait FromParent: Sized {
    type Parent;

    fn from_parent<'a>(parent: &'a Self::Parent, ident: &str) -> Result<&'a Self, String>;
    fn from_parent_mut<'a>(
        parent: &'a mut Self::Parent,
        ident: &str,
    ) -> Result<&'a mut Self, String>;
}

impl FromParent for ir::structures::peripheral::Peripheral {
    type Parent = Hal;

    fn from_parent<'a>(parent: &'a Self::Parent, ident: &str) -> Result<&'a Self, String> {
        parent
            .peripherals
            .get(ident)
            .ok_or(error!("peripheral [{}] does not exist.", ident.bold()))
    }

    fn from_parent_mut<'a>(
        parent: &'a mut Self::Parent,
        ident: &str,
    ) -> Result<&'a mut Self, String> {
        parent
            .peripherals
            .get_mut(ident)
            .ok_or(error!("peripheral [{}] does not exist.", ident.bold()))
    }
}

impl FromParent for ir::structures::register::Register {
    type Parent = ir::structures::peripheral::Peripheral;

    fn from_parent<'a>(parent: &'a Self::Parent, ident: &str) -> Result<&'a Self, String> {
        parent
            .registers
            .get(ident)
            .ok_or(error!("register [{}] does not exist.", ident.bold()))
    }

    fn from_parent_mut<'a>(
        parent: &'a mut Self::Parent,
        ident: &str,
    ) -> Result<&'a mut Self, String> {
        parent
            .registers
            .get_mut(ident)
            .ok_or(error!("register [{}] does not exist.", ident.bold()))
    }
}

impl FromParent for ir::structures::field::Field {
    type Parent = ir::structures::register::Register;

    fn from_parent<'a>(parent: &'a Self::Parent, ident: &str) -> Result<&'a Self, String> {
        parent
            .fields
            .get(ident)
            .ok_or(error!("field [{}] does not exist.", ident.bold()))
    }

    fn from_parent_mut<'a>(
        parent: &'a mut Self::Parent,
        ident: &str,
    ) -> Result<&'a mut Self, String> {
        parent
            .fields
            .get_mut(ident)
            .ok_or(error!("field [{}] does not exist.", ident.bold()))
    }
}

impl FromParent for ir::structures::variant::Variant {
    type Parent = ir::structures::field::Field;

    fn from_parent<'a>(parent: &'a Self::Parent, ident: &str) -> Result<&'a Self, String> {
        let Numericity::Enumerated { variants } = &parent.numericity else {
            Err(error!(
                "field [{}] is numeric and as such holds no variants.",
                parent.ident.bold()
            ))?
        };

        variants
            .get(ident)
            .ok_or(error!("variant [{}] does not exist.", ident.bold()))
    }

    fn from_parent_mut<'a>(
        parent: &'a mut Self::Parent,
        ident: &str,
    ) -> Result<&'a mut Self, String> {
        let Numericity::Enumerated { variants } = &mut parent.numericity else {
            Err(error!(
                "field [{}] is numeric and as such holds no variants.",
                parent.ident.bold()
            ))?
        };

        variants
            .get_mut(ident)
            .ok_or(error!("variant [{}] does not exist.", ident.bold()))
    }
}

pub trait Structure {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String>;
    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String>;

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String>;
}

impl Structure for Hal {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String> {
        self.peripherals
            .get(ident)
            .map(|p| (p as &dyn Structure).into())
            .ok_or(error!("peripheral [{}] does not exist.", ident.bold()))
    }

    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String> {
        self.peripherals
            .get_mut(ident)
            .map(|p| (p as &mut dyn Structure).into())
            .ok_or(error!("peripheral [{}] does not exist.", ident.bold()))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String> {
        self.peripherals
            .remove(ident)
            .map(|p| Box::new(p) as _)
            .ok_or(error!("peripheral [{}] does not exist.", ident.bold()))
    }
}

impl Structure for ir::structures::peripheral::Peripheral {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String> {
        self.registers
            .get(ident)
            .map(|p| (p as &dyn Structure).into())
            .ok_or(error!("register [{}] does not exist.", ident.bold()))
    }

    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String> {
        self.registers
            .get_mut(ident)
            .map(|p| (p as &mut dyn Structure).into())
            .ok_or(error!("register [{}] does not exist.", ident.bold()))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String> {
        self.registers
            .remove(ident)
            .map(|r| Box::new(r) as _)
            .ok_or(error!("register [{}] does not exist.", ident.bold()))
    }
}

impl Structure for ir::structures::register::Register {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String> {
        self.fields
            .get(ident)
            .map(|f| (f as &dyn Structure).into())
            .ok_or(error!("field [{}] does not exist.", ident.bold()))
    }

    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String> {
        self.fields
            .get_mut(ident)
            .map(|f| (f as &mut dyn Structure).into())
            .ok_or(error!("field [{}] does not exist.", ident.bold()))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String> {
        self.fields
            .remove(ident)
            .map(|f| Box::new(f) as _)
            .ok_or(error!("field [{}] does not exist.", ident.bold()))
    }
}

impl Structure for ir::structures::field::Field {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String> {
        let Numericity::Enumerated { variants } = &self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        variants
            .get(ident)
            .map(|v| (v as &dyn Structure).into())
            .ok_or(error!("variant [{}] does not exist.", ident.bold()))
    }

    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String> {
        let Numericity::Enumerated { variants } = &mut self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        variants
            .get_mut(ident)
            .map(|v| (v as &mut dyn Structure).into())
            .ok_or(error!("variant [{}] does not exist.", ident.bold()))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String> {
        let Numericity::Enumerated { variants } = &mut self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        variants
            .remove(ident)
            .map(|v| Box::new(v) as _)
            .ok_or(error!("variant [{}] does not exist.", ident.bold()))
    }
}

impl Structure for ir::structures::variant::Variant {
    fn get_child<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn Structure>, String> {
        Err(error!("variants do not have sub-structures."))
    }

    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<Box<&'a mut dyn Structure>, String> {
        Err(error!("variants do not have sub-structures."))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Box<dyn Structure>, String> {
        Err(error!("variants do not have sub-structures."))
    }
}
