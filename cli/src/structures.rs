use std::collections::HashMap;

use colored::Colorize;
use ir::structures::field::Numericity;

use crate::utils::feedback::error;

pub trait Structure: DynStructure {
    type Child;

    fn children(&self) -> Result<&HashMap<String, Self::Child>, String>;
    fn children_mut(&mut self) -> Result<&mut HashMap<String, Self::Child>, String>;

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
        self.get_child(child.ident()).err().ok_or(error!(
            "[{}] already exsts in [{}].",
            child.ident().bold(),
            self.ident().bold()
        ))?;

        self.children_mut()?.insert(child.ident().to_owned(), child);

        Ok(())
    }
}

pub trait DynStructure {
    fn ident(&self) -> &str;

    fn info(&self) -> String;

    fn get_child_boxed<'a>(&'a self, ident: &str) -> Result<Box<&'a dyn DynStructure>, String>;
    fn get_child_boxed_mut<'a>(
        &'a mut self,
        ident: &str,
    ) -> Result<Box<&'a mut dyn DynStructure>, String>;

    fn remove_child_boxed(&mut self, ident: &str) -> Result<Box<dyn DynStructure>, String>;
}

impl Structure for ir::structures::hal::Hal {
    type Child = ir::structures::peripheral::Peripheral;

    fn children(&self) -> Result<&HashMap<String, Self::Child>, String> {
        Ok(&self.peripherals)
    }
    fn children_mut(&mut self) -> Result<&mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.peripherals)
    }
}

impl DynStructure for ir::structures::hal::Hal {
    fn ident(&self) -> &str {
        "HAL"
    }

    fn info(&self) -> String {
        let min_addr = self
            .peripherals
            .values()
            .map(|peripheral| peripheral.base_addr)
            .min();

        let max_addr = self
            .peripherals
            .values()
            .max()
            .map(|peripheral| peripheral.base_addr + peripheral.width());

        let addr_space = {
            if let (Some(min_addr), Some(max_addr)) = (min_addr, max_addr) {
                let min_addr_str = format!("0x{:08x}", min_addr).bold();
                let max_addr_str = format!("0x{:08x}", max_addr).bold();

                format!("{}..{}", min_addr_str, max_addr_str,).into()
            } else {
                "-".bold()
            }
        };

        format!(
            "peripherals: {}\naddress space: {}",
            self.peripherals.len().to_string().bold(),
            addr_space,
        )
    }

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

    fn children(&self) -> Result<&HashMap<String, Self::Child>, String> {
        Ok(&self.registers)
    }

    fn children_mut(&mut self) -> Result<&mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.registers)
    }
}

impl DynStructure for ir::structures::peripheral::Peripheral {
    fn ident(&self) -> &str {
        &self.ident
    }

    fn info(&self) -> String {
        let max_addr = self.base_addr
            + self
                .registers
                .values()
                .max()
                .map(|register| register.offset + 4)
                .unwrap_or(0);

        let addr_space = format!(
            "address space: {}..{}",
            format!("0x{:08x}", self.base_addr).bold(),
            format!("0x{:08x}", max_addr).bold()
        );
        let entitlements = format!(
            "entitlements: [{}]",
            self.entitlements
                .iter()
                .map(|entitlement| entitlement.to_string().bold().to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let regsiters = format!("registers: {}", self.registers.len().to_string().bold());

        vec![addr_space, entitlements, regsiters].join("\n")
    }

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

    fn children(&self) -> Result<&HashMap<String, Self::Child>, String> {
        Ok(&self.fields)
    }

    fn children_mut(&mut self) -> Result<&mut HashMap<String, Self::Child>, String> {
        Ok(&mut self.fields)
    }
}

impl DynStructure for ir::structures::register::Register {
    fn ident(&self) -> &str {
        &self.ident
    }

    fn info(&self) -> String {
        let offset = format!("offset: {}", format!("0x{:02x}", self.offset).bold());
        let fields = format!("fields: {}", self.fields.len().to_string().bold());

        vec![offset, fields].join("\n")
    }

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

    fn children(&self) -> Result<&HashMap<String, Self::Child>, String> {
        let Numericity::Enumerated { variants } = &self.numericity else {
            Err(error!(
                "field [{}] is numeric and as such has no variants.",
                self.ident.bold()
            ))?
        };

        Ok(variants)
    }

    fn children_mut(&mut self) -> Result<&mut HashMap<String, Self::Child>, String> {
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
    fn ident(&self) -> &str {
        &self.ident
    }

    fn info(&self) -> String {
        let offset = format!(
            "offset: {}",
            format!("0x{:02x} ({})", self.offset, self.offset).bold()
        );

        let (numericity, variants) = match &self.numericity {
            Numericity::Numeric => ("numeric".bold().to_string(), None),
            Numericity::Enumerated { variants } => (
                "enumerated".bold().to_string(),
                Some(format!("variants: {}", variants.len().to_string().bold())),
            ),
        };

        let numericity = format!("numericity: {}", numericity);

        vec![Some(offset), Some(numericity), variants]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("\n")
    }

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
    fn ident(&self) -> &str {
        &self.ident
    }

    fn info(&self) -> String {
        format!("bit value: {}", self.bits.to_string().bold())
    }

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
