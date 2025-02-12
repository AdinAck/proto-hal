use std::{collections::HashMap, path::PathBuf};

use crate::{repl::Repl, utils::feedback::error};
use clap::Subcommand;
use colored::Colorize;
use enum_dispatch::enum_dispatch;
use field::Field;
use ir::structures::field::Numericity;
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

pub trait Structure {
    type Child;

    fn ident(&self) -> &str;

    fn children<'a>(&'a self) -> Result<&'a HashMap<String, Self::Child>, String>;
    fn children_mut<'a>(&'a mut self) -> Result<&'a mut HashMap<String, Self::Child>, String>;

    fn get_child<'a>(&'a self, ident: &str) -> Result<&'a Self::Child, String> {
        self.children()?.get(ident).ok_or(error!(
            "[{}] does not exist in [{}].",
            ident.bold(),
            self.ident().bold()
        ))
    }
    fn get_child_mut<'a>(&'a mut self, ident: &str) -> Result<&'a mut Self::Child, String> {
        let current_ident = self.ident().bold();
        self.children_mut()?.get_mut(ident).ok_or(error!(
            "[{}] does not exist in [{}].",
            ident.bold(),
            current_ident,
        ))
    }

    fn remove_child(&mut self, ident: &str) -> Result<Self::Child, String> {
        self.children_mut()?.remove(ident).ok_or(error!(
            "[{}] does not exist in [{}].",
            ident.bold(),
            self.ident().bold()
        ))
    }
    fn push_child(&mut self, child: Self::Child) -> Result<(), String>
    where
        Self::Child: Structure,
    {
        self.get_child(&child.ident()).err().ok_or(error!(
            "[{}] already exsts in [{}].",
            child.ident().bold(),
            self.ident().bold()
        ))?;

        self.children_mut()?.insert(child.ident().to_owned(), child);

        Ok(())
    }
}

pub trait DynStructure {
    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String>;
    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String>;

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String>;
}

impl Structure for ir::structures::hal::Hal {
    type Child = ir::structures::peripheral::Peripheral;

    fn ident(&self) -> &str {
        "HAL"
    }

    fn children<'a>(&'a self) -> Result<&'a HashMap<String, Self::Child>, String> {
        Ok(&self.peripherals)
    }
    fn children_mut<'a>(&'a mut self) -> Result<&'a mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.peripherals)
    }
}

impl DynStructure for ir::structures::hal::Hal {
    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String> {
        <Self as Structure>::get_child(self, ident).map(|s| (s as &dyn DynStructure).into())
    }

    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String> {
        <Self as Structure>::get_child_mut(self, ident).map(|s| (s as &mut dyn DynStructure).into())
    }

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String> {
        <Self as Structure>::remove_child(self, ident).map(|s| Box::new(s) as _)
    }
}

impl Structure for ir::structures::peripheral::Peripheral {
    type Child = ir::structures::register::Register;

    fn ident(&self) -> &str {
        &self.ident
    }

    fn children<'a>(&'a self) -> Result<&'a HashMap<String, Self::Child>, String> {
        Ok(&self.registers)
    }

    fn children_mut<'a>(&'a mut self) -> Result<&'a mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.registers)
    }
}

impl DynStructure for ir::structures::peripheral::Peripheral {
    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String> {
        <Self as Structure>::get_child(self, ident).map(|s| (s as &dyn DynStructure).into())
    }

    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String> {
        <Self as Structure>::get_child_mut(self, ident).map(|s| (s as &mut dyn DynStructure).into())
    }

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String> {
        <Self as Structure>::remove_child(self, ident).map(|s| Box::new(s) as _)
    }
}

impl Structure for ir::structures::register::Register {
    type Child = ir::structures::field::Field;

    fn ident(&self) -> &str {
        &self.ident
    }

    fn children<'a>(&'a self) -> Result<&'a HashMap<String, Self::Child>, String> {
        Ok(&self.fields)
    }

    fn children_mut<'a>(&'a mut self) -> Result<&'a mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.fields)
    }
}

impl DynStructure for ir::structures::register::Register {
    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String> {
        <Self as Structure>::get_child(self, ident).map(|s| (s as &dyn DynStructure).into())
    }

    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String> {
        <Self as Structure>::get_child_mut(self, ident).map(|s| (s as &mut dyn DynStructure).into())
    }

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String> {
        <Self as Structure>::remove_child(self, ident).map(|s| Box::new(s) as _)
    }
}

impl Structure for ir::structures::field::Field {
    type Child = ir::structures::variant::Variant;

    fn ident(&self) -> &str {
        &self.ident
    }

    fn children<'a>(&'a self) -> Result<&'a HashMap<String, Self::Child>, String> {
        let Numericity::Enumerated { variants } = &self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        Ok(variants)
    }

    fn children_mut<'a>(&'a mut self) -> Result<&'a mut HashMap<String, Self::Child>, String> {
        let Numericity::Enumerated { variants } = &mut self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        Ok(variants)
    }
}

impl DynStructure for ir::structures::field::Field {
    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String> {
        <Self as Structure>::get_child(self, ident).map(|s| (s as &dyn DynStructure).into())
    }

    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String> {
        <Self as Structure>::get_child_mut(self, ident).map(|s| (s as &mut dyn DynStructure).into())
    }

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String> {
        <Self as Structure>::remove_child(self, ident).map(|s| Box::new(s) as _)
    }
}

impl DynStructure for ir::structures::variant::Variant {
    fn get_child_boxed<'a>(
        &'a self,
        #[allow(unused)] ident: &str,
    ) -> Result<Box<&'a dyn DynStructure>, String> {
        Err(error!("variants have no sub-structures."))
    }

    fn get_child_boxed_mut<'a>(
        &'a mut self,
        #[allow(unused)] ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String> {
        Err(error!("variants have no sub-structures."))
    }

    fn remove_child_boxed(
        &mut self,
        #[allow(unused)] ident: &str,
    ) -> Result<Box<dyn DynStructure>, String> {
        Err(error!("variants have no sub-structures."))
    }
}
