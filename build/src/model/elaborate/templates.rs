//! Template resolution: definitions derive from other definitions.

use syntax::ast::{
    Device, Peripheral, Register, Spanned,
};

use crate::model::semantic::Diagnostic;

use super::{
    Context, TEMPLATE_DEPTH_LIMIT, Templates, did_you_mean, merge,
};

impl<'ast, 'src> Context<'ast, 'src> {
    pub(super) fn resolve_device(&mut self, device: &Device<'src>) -> Option<Device<'src>> {
        let Some(template_ref) = device.head.template.clone() else {
            return Some(device.clone());
        };

        let template = self.resolve_template(
            &template_ref,
            "device",
            |templates, name| templates.devices.get(name).copied(),
            |templates| templates.devices.keys().map(|name| name.to_string()).collect(),
        )?;
        self.used_devices.insert(template as *const Device as usize);
        let template = self.resolve_device_inner(template, 1)?;

        Some(merge::device(template, device))
    }

    pub(super) fn resolve_device_inner(
        &mut self,
        device: &'ast Device<'src>,
        depth: u32,
    ) -> Option<Device<'src>> {
        let Some(template_ref) = device.head.template.clone() else {
            return Some(device.clone());
        };

        if depth > TEMPLATE_DEPTH_LIMIT {
            self.diagnostics
                .push(Diagnostic::template_depth_exceeded(template_ref.span));
            return None;
        }

        let template = self.resolve_template(
            &template_ref,
            "device",
            |templates, name| templates.devices.get(name).copied(),
            |templates| templates.devices.keys().map(|name| name.to_string()).collect(),
        )?;
        self.used_devices.insert(template as *const Device as usize);
        let template = self.resolve_device_inner(template, depth + 1)?;

        Some(merge::device(template, device))
    }

    pub(super) fn resolve_peripheral(&mut self, peripheral: &Peripheral<'src>) -> Option<Peripheral<'src>> {
        let Some(template_ref) = peripheral.head.template.clone() else {
            return Some(peripheral.clone());
        };

        let template = self.resolve_template(
            &template_ref,
            "peripheral",
            |templates, name| templates.peripherals.get(name).copied(),
            |templates| templates.peripherals.keys().map(|name| name.to_string()).collect(),
        )?;
        let template = self.resolve_peripheral_inner(template, 1)?;

        Some(merge::peripheral(template, peripheral))
    }

    pub(super) fn resolve_peripheral_inner(
        &mut self,
        peripheral: &'ast Peripheral<'src>,
        depth: u32,
    ) -> Option<Peripheral<'src>> {
        let Some(template_ref) = peripheral.head.template.clone() else {
            return Some(peripheral.clone());
        };

        if depth > TEMPLATE_DEPTH_LIMIT {
            self.diagnostics
                .push(Diagnostic::template_depth_exceeded(template_ref.span));
            return None;
        }

        let template = self.resolve_template(
            &template_ref,
            "peripheral",
            |templates, name| templates.peripherals.get(name).copied(),
            |templates| templates.peripherals.keys().map(|name| name.to_string()).collect(),
        )?;
        let template = self.resolve_peripheral_inner(template, depth + 1)?;

        Some(merge::peripheral(template, peripheral))
    }

    pub(super) fn resolve_register(&mut self, register: &Register<'src>) -> Option<Register<'src>> {
        let Some(template_ref) = register.head.template.clone() else {
            return Some(register.clone());
        };

        let template = self.resolve_template(
            &template_ref,
            "register",
            |templates, name| templates.registers.get(name).copied(),
            |templates| templates.registers.keys().map(|name| name.to_string()).collect(),
        )?;
        let template = self.resolve_register_inner(template, 1)?;

        Some(merge::register(template, register))
    }

    pub(super) fn resolve_register_inner(
        &mut self,
        register: &'ast Register<'src>,
        depth: u32,
    ) -> Option<Register<'src>> {
        let Some(template_ref) = register.head.template.clone() else {
            return Some(register.clone());
        };

        if depth > TEMPLATE_DEPTH_LIMIT {
            self.diagnostics
                .push(Diagnostic::template_depth_exceeded(template_ref.span));
            return None;
        }

        let template = self.resolve_template(
            &template_ref,
            "register",
            |templates, name| templates.registers.get(name).copied(),
            |templates| templates.registers.keys().map(|name| name.to_string()).collect(),
        )?;
        let template = self.resolve_register_inner(template, depth + 1)?;

        Some(merge::register(template, register))
    }

    /// Resolve a template reference against the referencing file: a single
    /// segment names the file's own top-level definitions, and `alias.name`
    /// reaches through one of its imports.
    ///
    /// `names` supplies the candidate set of the searched kind, for
    /// suggestions when the reference misses.
    pub(super) fn resolve_template<T>(
        &mut self,
        reference: &Spanned<syntax::ast::Path<'src>>,
        what: &str,
        get: impl FnOnce(&Templates<'ast, 'src>, &str) -> Option<(T, syntax::ast::Span)>,
        names: impl FnOnce(&Templates<'ast, 'src>) -> Vec<String>,
    ) -> Option<T> {
        let file = reference.span.context;

        match reference.inner.segments.as_slice() {
            [name] => {
                let found = get(&self.templates[file], name.inner);

                match &found {
                    Some((.., site)) => self.definitions.push((reference.span, *site)),
                    None => self.diagnostics.push(Diagnostic::unknown_template(
                        reference.span,
                        what,
                        name.inner,
                        None,
                        did_you_mean(name.inner, names(&self.templates[file])),
                    )),
                }

                found.map(|(template, ..)| template)
            }
            [alias, name] => {
                let Some(&target) = self.units[file].imports.get(alias.inner) else {
                    self.diagnostics.push(Diagnostic::unknown_import(
                        alias.span,
                        alias.inner,
                        did_you_mean(
                            alias.inner,
                            self.units[file].imports.keys().cloned().collect::<Vec<_>>(),
                        ),
                    ));
                    return None;
                };

                let found = get(&self.templates[target], name.inner);

                match &found {
                    Some((.., site)) => self.definitions.push((reference.span, *site)),
                    None => self.diagnostics.push(Diagnostic::unknown_template(
                        name.span,
                        what,
                        name.inner,
                        Some(alias.inner),
                        did_you_mean(name.inner, names(&self.templates[target])),
                    )),
                }

                found.map(|(template, ..)| template)
            }
            _ => {
                self.diagnostics
                    .push(Diagnostic::invalid_template_reference(reference.span));
                None
            }
        }
    }
}
