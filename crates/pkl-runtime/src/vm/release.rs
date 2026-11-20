//! Release host module data and typed instantiation.
use super::*;
use crate::heap::CollectionKind;

impl Vm {
    pub(super) fn release_current(&mut self) -> Result<(), RuntimeError> {
        // If we cooperatively resumed after initializing `pkl:semver`, discard
        // the module value left on the operand stack by its initializer frame.
        let base = self
            .frames
            .last()
            .ok_or_else(|| invalid("release current frame"))?
            .base;
        while self.stack.len() > base {
            self.pop()?;
        }

        if self.platform_metadata.is_none() {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "This standard-library current value requires explicit platform/release Host data.",
            ));
        }

        // 1. Ensure `pkl:semver` is initialized cooperatively before reading `Version`.
        let semver_module_id = match self.modules_stdlib.get("pkl:semver").cloned() {
            Some(modules::CachedModule::Value(id)) => id,
            Some(modules::CachedModule::Failed(error)) => return Err(error),
            None => {
                self.frames
                    .last_mut()
                    .ok_or_else(|| invalid("release current frame"))?
                    .pc -= 1;
                self.import_stdlib("pkl:semver")?;
                return Ok(());
            }
        };

        let module_id = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
            .and_then(|v| v.heap_id())
            .or_else(|| {
                if let Some(modules::CachedModule::Value(id)) =
                    self.modules_stdlib.get("pkl:release")
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .ok_or_else(|| invalid("release module receiver"))?;

        let version_class = self.get_or_create_module_class(semver_module_id, "Version")?;
        let version_obj = self.instantiate_typed(
            version_class,
            vec![
                ("major", Value::Int(0.into())),
                ("minor", Value::Int(32.into())),
                ("patch", Value::Int(0.into())),
                ("preRelease", Value::Null),
                ("build", Value::Null),
            ],
        )?;

        // 2. SourceCode
        let sc_class = self.get_or_create_module_class(module_id, "SourceCode")?;
        let sc = self.instantiate_typed(
            sc_class,
            vec![("homepage", Value::string("https://github.com/apple/pkl/"))],
        )?;

        // 3. Documentation
        let doc_class = self.get_or_create_module_class(module_id, "Documentation")?;
        let doc = self.instantiate_typed(
            doc_class,
            vec![(
                "homepage",
                Value::string("https://pkl-lang.org/main/0.32.0/"),
            )],
        )?;

        // 4. StandardLibrary with Set of modules
        let set_builder = self.heap.allocate(Object {
            collection_kind: CollectionKind::Set,
            ..Default::default()
        });
        self.frames.last_mut().unwrap().builders.push(set_builder);
        for line in include_str!("../stdlib/modules.txt").lines() {
            let line = line.trim();
            if !line.is_empty() {
                self.stack.extend([Value::string(line), Value::Bool(false)]);
                self.collection_instruction(Instruction::AddSetElement)?;
            }
        }
        self.frames.last_mut().unwrap().builders.pop();
        let set_obj = self.finish_strict_collection(set_builder)?;

        let stdlib_class = self.get_or_create_module_class(module_id, "StandardLibrary")?;
        let stdlib =
            self.instantiate_typed(stdlib_class, vec![("modules", Value::Object(set_obj))])?;

        let platform_meta = self.platform_metadata.as_ref().unwrap();
        let os_name = platform_meta.os_name.to_java_utf8();
        let os_version = platform_meta.os_version.to_java_utf8();
        let java_version = platform_meta.java_version.to_java_utf8();
        let flavor = if java_version.is_empty() || java_version == "native" {
            "native".to_owned()
        } else {
            format!("Java {java_version}")
        };
        let version_info = format!("Pkl 0.32.0 ({os_name} {os_version}, {flavor})");

        // 5. Release
        let rel_class = self.get_or_create_module_class(module_id, "Release")?;
        let rel = self.instantiate_typed(
            rel_class,
            vec![
                ("version", Value::Object(version_obj)),
                ("versionInfo", Value::string(version_info)),
                ("commitId", Value::string("e51fb2f")),
                ("sourceCode", Value::Object(sc)),
                ("documentation", Value::Object(doc)),
                ("standardLibrary", Value::Object(stdlib)),
            ],
        )?;

        self.stack.push(Value::Object(rel));
        Ok(())
    }
}
