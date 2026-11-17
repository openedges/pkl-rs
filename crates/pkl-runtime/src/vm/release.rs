//! Release host module data and typed instantiation.
use super::*;
use crate::heap::CollectionKind;

impl Vm {
    pub(super) fn release_current(&mut self) -> Result<(), RuntimeError> {
        if self.platform_metadata.is_none() {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "This standard-library current value requires explicit platform/release Host data.",
            ));
        }
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

        // 1. semver.Version from pkl:semver
        let semver_module_id = match self.modules_stdlib.get("pkl:semver") {
            Some(modules::CachedModule::Value(id)) => *id,
            _ => {
                // If not yet imported, import it
                self.import_stdlib("pkl:semver")?;
                match self.modules_stdlib.get("pkl:semver") {
                    Some(modules::CachedModule::Value(id)) => *id,
                    _ => return Err(invalid("missing pkl:semver module")),
                }
            }
        };
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
        let os_name = platform_meta.os_name.as_str().unwrap_or("macOS");
        let os_version = platform_meta.os_version.as_str().unwrap_or("unknown");
        let version_info = format!("Pkl 0.32.0 ({os_name} {os_version}, pkl-rust 0.32.0)");

        // 5. Release
        let rel_class = self.get_or_create_module_class(module_id, "Release")?;
        let rel = self.instantiate_typed(
            rel_class,
            vec![
                ("version", Value::Object(version_obj)),
                ("versionInfo", Value::string(version_info)),
                (
                    "commitId",
                    Value::string("e51fb2f9d5040ffb08bbcd8d1e1f79b3628004f0"),
                ),
                ("sourceCode", Value::Object(sc)),
                ("documentation", Value::Object(doc)),
                ("standardLibrary", Value::Object(stdlib)),
            ],
        )?;

        self.stack.push(Value::Object(rel));
        Ok(())
    }
}
