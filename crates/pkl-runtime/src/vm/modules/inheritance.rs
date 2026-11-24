//! Module inheritance uses the already published receiver and ordinary import
//! continuations. An amendment adopts a class; an extending module defines one.
use super::*;

impl Vm {
    pub(in crate::vm) fn check_class_instantiable(
        &self,
        class: &Class,
    ) -> Result<(), RuntimeError> {
        if !class.abstract_ {
            return Ok(());
        }
        let is_module = self.heap.find_class(class.id).is_some_and(|id| {
            self.heap
                .get_class(id)
                .and_then(|class| self.heap.get(class.prototype))
                .is_ok_and(|object| object.is_module)
        });
        let name = if is_module || builtins::name(class.id).is_some() {
            class.name.clone()
        } else {
            class
                .module
                .as_ref()
                .unwrap_or(&self.program.module_name)
                .concat(&"#".into())
                .concat(&class.name)
        };
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            PklString::from("Cannot instantiate abstract class `")
                .concat(&name)
                .concat(&"`.".into()),
        );
        error.hint = Some(
            "Abstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass."
                .into(),
        );
        Err(error)
    }

    pub(in crate::vm) fn initialize_module_parent(&mut self) -> Result<(), RuntimeError> {
        let parent = self.pop()?.as_object()?;
        let receiver = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing module receiver"))?
            .as_object()?;
        let unit = self
            .heap
            .get(receiver)?
            .module_unit
            .ok_or_else(|| invalid("missing module unit"))?;
        let unit = self.program.units[unit.0 as usize].clone();
        if receiver == parent {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Module `{}` cannot {} itself.",
                    unit.name,
                    if unit.mode == ModuleMode::Amends {
                        "amend"
                    } else {
                        "extend"
                    }
                ),
            ));
        }
        if !self.heap.get(parent)?.is_module {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                "Module parent must be a module.",
            ));
        }
        match unit.mode {
            ModuleMode::Plain => return Err(invalid("plain module has no explicit parent")),
            ModuleMode::Extends => {
                let object = self.heap.get(parent)?;
                let class = collections::typed_class(&self.heap, parent)?
                    .ok_or_else(|| invalid("module parent has no class"))?;
                if object.class_definition.is_none() {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Module `{}` cannot be extended or used as type because it amends another module.",
                            class.name
                        ),
                    ));
                }
                if !class.open && !class.abstract_ {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!("Cannot extend non-open module `{}`.", class.name),
                    );
                    error.hint = Some("By default, modules are closed for extension.\nTo make a module extensible, add an `open` modifier: `open module my_module; ...`".into());
                    return Err(error);
                }
                let class = self
                    .heap
                    .object_class(receiver)?
                    .ok_or_else(|| invalid("extending module class"))?;
                let superclass = self
                    .heap
                    .object_class(parent)?
                    .ok_or_else(|| invalid("supermodule class"))?;
                self.heap.link_superclass(class, Some(superclass))?;
            }
            ModuleMode::Amends => {
                let class = collections::typed_class(&self.heap, parent)?
                    .ok_or_else(|| invalid("module parent has no class"))?;
                self.check_class_instantiable(&class)?;
                self.validate_object_members(parent, &self.program.objects[unit.template])?;
                let members = self.program.objects[unit.template].clone();
                let class = self.heap.object_class(parent)?;
                let module = self.heap.get_mut(receiver)?;
                module.instance_class = class;
                module.is_module = true;
                module.parent = Some(parent);
                module.members = members.into();
                module.published_module_members =
                    module.members.iter().map(|member| member.code).collect();
            }
        }
        // Linking a module parent mutates the published receiver/class hierarchy in
        // place (e.g., amending module adopts its parent's Typed class), invalidating
        // any cached strict-collection type checks that observed it earlier.
        self.verified_strict_collections.clear();
        Ok(())
    }
}
