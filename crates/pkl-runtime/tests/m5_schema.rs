//! Integration tests for pure-Rust Wasm-compatible `ModuleSchema` export (`pkl-runtime::schema`).

use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequest,
    ModuleRequestKind, ModuleSource, ModuleSourceSpec, ResolvedModule, ResolvedModuleKey,
    default_completion,
};
use pkl_runtime::pkl_binary_decode::DecodedValue;
use pkl_runtime::schema::{Modifier, ModuleSchema, PType, Variance, extract_module_schema_with};
use pkl_runtime::value::{PklString, RuntimeError, Value};
use std::collections::BTreeMap;

fn drive_in_memory(
    session: &mut SourceSession,
    files: &BTreeMap<String, String>,
) -> Result<Value, RuntimeError> {
    loop {
        match session.poll(0, 65_536) {
            HostPoll::Ready(res) => return res,
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                let completion = handle_mem_module(&request, files);
                session.complete_module(&token, completion, 0)?;
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0)?;
            }
            HostPoll::Glob { token, .. } => {
                session.complete_default_glob(&token, &[], &[], 0)?;
            }
            HostPoll::Log { token, .. } => {
                session.acknowledge_log(&token, Ok(()), 0)?;
            }
            HostPoll::TransformFrame { token, frame } => {
                session.complete_frame_transform(&token, Ok((*frame).clone()), 0)?;
            }
        }
    }
}

fn handle_mem_module(
    request: &ModuleRequest,
    files: &BTreeMap<String, String>,
) -> ModuleCompletion {
    if let Some(completion) = default_completion(request) {
        return completion;
    }
    match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, .. } => {
            let kind = if uri.starts_with("pkl:") {
                ModuleKeyKind::StandardLibrary
            } else {
                ModuleKeyKind::Host
            };
            ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind,
                handle: "mem".into(),
            }))
        }
        ModuleRequestKind::QueryCachePolicy { .. } => ModuleCompletion::CachePolicy(Ok(true)),
        ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id,
            ..
        } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
            id: *resolved_key_id,
            key: key.clone(),
            handle: key.handle.clone(),
            module: ResolvedModule {
                original_uri: key.uri.clone(),
                resolved_uri: key.uri.clone(),
                source_uri: key.uri.clone(),
            },
        })),
        ModuleRequestKind::Capabilities { .. } => {
            ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                local: true,
                hierarchical: true,
                fragment_paths: false,
            }))
        }
        ModuleRequestKind::HasElement { element_uri, .. } => {
            ModuleCompletion::HasElement(Ok(files.contains_key(element_uri.as_str().unwrap_or(""))))
        }
        ModuleRequestKind::Resolve { original_uri, .. } => {
            ModuleCompletion::Resolved(Ok(ResolvedModule {
                original_uri: original_uri.clone(),
                resolved_uri: original_uri.clone(),
                source_uri: original_uri.clone(),
            }))
        }
        ModuleRequestKind::LoadSource { resolved, .. } => {
            let uri = resolved.original_uri.as_str().unwrap_or("");
            let text = files
                .get(uri)
                .unwrap_or_else(|| panic!("missing in-memory module {uri}"));
            let inferred_name = uri
                .rsplit('/')
                .next()
                .unwrap_or("module")
                .strip_suffix(".pkl")
                .unwrap_or("module");
            ModuleCompletion::Source(Ok(ModuleSource {
                text: text.clone().into(),
                inferred_name: inferred_name.into(),
            }))
        }
        other => panic!("unexpected module request: {other:?}"),
    }
}

fn eval_schema_mem(uri: &str, files: &BTreeMap<String, String>) -> ModuleSchema {
    let mut session = SourceSession::module_evaluator(SourceSessionOptions::default());
    let spec = ModuleSourceSpec {
        uri: PklString::from(uri),
        contents: files.get(uri).cloned().map(Into::into),
    };
    session.start_load_module(spec, 0, None).unwrap();
    let module = drive_in_memory(&mut session, files).unwrap();
    extract_module_schema_with(&mut session, module, 0, None, |s| drive_in_memory(s, files))
        .unwrap()
}

#[test]
fn exports_classes_properties_methods_constraints_annotations_and_locations() {
    let mut files = BTreeMap::new();
    files.insert(
        "file:///sample.pkl".to_string(),
        r#"/// Sample module docs.
///
/// Second paragraph.
@ModuleInfo { minPklVersion = "0.25.0" }
open module com.example.sample

/// A person class.
@Deprecated {
  message = "Use NewPerson"
  replaceWith = "NewPerson"
}
open class Person {
  /// Person name.
  @AlsoKnownAs { names { "fullName"; "label" } }
  name: String(length >= 3, !isBlank)

  hidden age: Int = 42

  /// Recursive reference to test cycle safety.
  friend: Person?

  /// Greet someone.
  function greet(_: String, times: Int): "hi"|"bye" = "hi"
}

abstract class BaseEntity {
  /// Base id docs.
  id: String
}

class ChildEntity extends BaseEntity {
  id: String
  extra: List<Person>
}

typealias Email = String(contains("@"))

typealias PairList<out T> = List<Pair<T, String>>

/// Module prop.
fixed count: Int = 1

output {
  value = count
}
"#
        .to_string(),
    );

    let schema = eval_schema_mem("file:///sample.pkl", &files);
    assert_eq!(schema.module_name, "com.example.sample");
    assert_eq!(schema.module_uri, "file:///sample.pkl");
    assert!(!schema.is_amend);
    assert!(!schema.is_extend());
    assert!(schema.supermodule.is_none());
    assert_eq!(
        schema.doc_comment.as_deref(),
        Some("Sample module docs.\n\nSecond paragraph.")
    );
    assert_eq!(schema.annotations.len(), 1);
    assert_eq!(schema.annotations[0].class_info.simple_name, "ModuleInfo");

    // Module properties: `count` is declared; `output { ... }` is untyped override -> stays on pkl.base#Module!
    let mod_cls = schema.module_class();
    assert_eq!(mod_cls.modifiers, vec![Modifier::Open]);
    let own_prop_names: Vec<&str> = mod_cls.properties.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(own_prop_names, vec!["count"]);
    let count_prop = mod_cls.property("count").unwrap();
    // `fixed` is NOT in org.pkl.core.Modifier!
    assert!(count_prop.modifiers.is_empty());

    // `all_properties` includes `output` from `pkl.base#Module` first, then `count`
    let all_mod_props = schema.store.all_properties(schema.module_class_id);
    assert_eq!(all_mod_props.len(), 2);
    assert_eq!(all_mod_props[0].0, "output");
    assert_eq!(all_mod_props[0].1.owner_info.module_name, "pkl.base");
    assert_eq!(all_mod_props[1].0, "count");
    assert_eq!(
        all_mod_props[1].1.owner_info.module_name,
        "com.example.sample"
    );

    // Person class
    let person = schema.class_by_name("Person").unwrap();
    assert_eq!(person.modifiers, vec![Modifier::Open]);
    assert_eq!(person.doc_comment.as_deref(), Some("A person class."));
    assert_eq!(person.annotations.len(), 1);
    assert_eq!(person.annotations[0].class_info.simple_name, "Deprecated");
    assert_eq!(
        person.annotations[0].get_property("message"),
        Some(&DecodedValue::String("Use NewPerson".into()))
    );
    assert_eq!(
        person.annotations[0].get_property("replaceWith"),
        Some(&DecodedValue::String("NewPerson".into()))
    );

    // Constrained property preserves exact constraint source strings
    let name_prop = person.property("name").unwrap();
    match &name_prop.ty {
        PType::Constrained {
            base_type,
            constraints,
        } => {
            assert_eq!(constraints, &vec!["length >= 3", "!isBlank"]);
            match base_type.as_ref() {
                PType::Class { class_info, .. } => {
                    assert_eq!(class_info.module_name, "pkl.base");
                    assert_eq!(class_info.simple_name, "String");
                }
                other => panic!("expected String class, got {other:?}"),
            }
        }
        other => panic!("expected constrained type, got {other:?}"),
    }

    // Unnamed parameter `_` becomes `_#0`
    let greet = person.method("greet").unwrap();
    assert_eq!(greet.parameters.len(), 2);
    assert_eq!(greet.parameters[0].0, "_#0");
    assert_eq!(greet.parameters[1].0, "times");
    assert_eq!(
        greet.return_type,
        PType::Union(vec![
            PType::StringLiteral("hi".to_string()),
            PType::StringLiteral("bye".to_string()),
        ])
    );

    // Cyclic reference `friend: Person?` resolves cleanly to Person's own ClassId
    let friend_prop = person.property("friend").unwrap();
    match &friend_prop.ty {
        PType::Nullable(inner) => match inner.as_ref() {
            PType::Class { class_id, .. } => assert_eq!(*class_id, person.id),
            other => panic!("expected Person class, got {other:?}"),
        },
        other => panic!("expected nullable, got {other:?}"),
    }

    // Inherited doc comment on ChildEntity.id comes from BaseEntity.id
    let child = schema.class_by_name("ChildEntity").unwrap();
    assert_eq!(
        schema.store.inherited_property_doc_comment(child.id, "id"),
        Some("Base id docs.")
    );
    assert!(child.source_location.start_line < child.source_location.end_line);

    // Type aliases with variance and constraints
    let pair_list = schema.type_alias_by_name("PairList").unwrap();
    assert_eq!(pair_list.type_parameters.len(), 1);
    assert_eq!(pair_list.type_parameters[0].name, "T");
    assert_eq!(pair_list.type_parameters[0].variance, Variance::Covariant);
}

#[test]
fn exports_module_inheritance_and_cross_module_imports() {
    let mut files = BTreeMap::new();
    files.insert(
        "file:///shared.pkl".to_string(),
        r#"module com.example.shared

class SharedItem {
  label: String
}
"#
        .to_string(),
    );
    files.insert(
        "file:///base_mod.pkl".to_string(),
        r#"open module com.example.baseMod

baseProp: Int = 1

class BaseCls
typealias BaseAlias = String
"#
        .to_string(),
    );
    files.insert(
        "file:///child_mod.pkl".to_string(),
        r#"extends "file:///base_mod.pkl"

import "file:///shared.pkl"

childProp: shared.SharedItem
modTyped: shared
class ChildCls
"#
        .to_string(),
    );

    let schema = eval_schema_mem("file:///child_mod.pkl", &files);
    assert!(schema.is_extend());
    assert!(!schema.is_amend);
    assert_eq!(
        schema.supermodule.as_ref().unwrap().module_name,
        "com.example.baseMod"
    );
    assert_eq!(
        schema.imports,
        vec![("shared".to_string(), "file:///shared.pkl".to_string())]
    );

    // all_classes has supermodule class first, then child class
    let all_cls: Vec<String> = schema.all_classes().into_iter().map(|(n, _)| n).collect();
    assert_eq!(all_cls, vec!["BaseCls", "ChildCls"]);

    // all_type_aliases inherits BaseAlias
    let all_ta: Vec<String> = schema
        .all_type_aliases()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert_eq!(all_ta, vec!["BaseAlias"]);

    // Cross-module type reference + module-as-type reference
    let child_prop = schema.module_class().property("childProp").unwrap();
    match &child_prop.ty {
        PType::Class { class_info, .. } => {
            assert_eq!(class_info.module_name, "com.example.shared");
            assert_eq!(class_info.simple_name, "SharedItem");
        }
        other => panic!("expected SharedItem, got {other:?}"),
    }
    let mod_typed = schema.module_class().property("modTyped").unwrap();
    match &mod_typed.ty {
        PType::Class { class_info, .. } => {
            assert_eq!(class_info.module_name, "com.example.shared");
            assert!(class_info.is_module_class);
        }
        other => panic!("expected shared module class, got {other:?}"),
    }
}

#[test]
fn exports_pkl_base_stdlib_schema_fast_without_reflect() {
    let files = BTreeMap::new();
    let schema = eval_schema_mem("pkl:base", &files);
    assert_eq!(schema.module_name, "pkl.base");
    assert!(schema.class_by_name("Any").is_some());
    assert!(schema.class_by_name("String").is_some());
    assert!(schema.type_alias_by_name("NonNull").is_some());
    assert!(schema.type_alias_by_name("UInt").is_some());
    // Builtin class `Any` has no superclass and declares `toString`, `getClass`, `ifNonNull`
    let any_cls = schema.class_by_name("Any").unwrap();
    assert!(any_cls.superclass.is_none());
    assert!(any_cls.method("toString").is_some());
    assert!(any_cls.method("ifNonNull").is_some());
}
