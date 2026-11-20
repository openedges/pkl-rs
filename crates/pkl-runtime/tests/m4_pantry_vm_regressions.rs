//! Regression tests for Wave 2 Pantry VM fixes:
//! - Bug 3A (`vm/equality.rs`): module/typed object equality & hashing excluding
//!   builtin hidden properties (`Module.output`) and prototype `class`/`typealias`
//!   definitions (`pkl.experimental.uri/tests/URI.pkl:59`).
//! - Bug 3B (`vm/modules.rs` + `vm.rs`): mutually cyclic cached module imports
//!   during module/class initialization (`pkl.tar/tar.pkl <-> File.pkl`), while
//!   preserving genuine type-alias, property, and uncached-module cycle errors.
//! - Bug A (`vm.rs`): `VmDynamic` non-constant and `Typed.toDynamic()` delegating
//!   `default` property reads skip `Dynamic.default: (unknown) -> Any` checks
//!   (`org.openapis.v3.contrib` `SampleAPI.pkl` & `SwallowSchema.pkl`), while
//!   constant `Dynamic` members and `Listing`/`Mapping` members keep checking.
//! - Bug B (`vm/equality.rs`): `VmTyped.equals` compares `class_identity` without
//!   requiring matching `is_module` (`Map(...).toTyped(OpenMod.getClass()) == new OpenMod {}`,
//!   `org.json_schema.contrib/tests/ref.pkl`).

use pkl_runtime::evaluation::frontend::{
    LosslessModuleSourceInput, ModuleEvalOptions, evaluate_module_output_with_hosts,
};
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyKind, ModuleRequest, ModuleRequestKind,
    ModuleSource, ResolvedModule, ResolvedModuleKey, default_completion,
};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;

fn eval_virtual_modules(
    primary_uri: &str,
    modules: &BTreeMap<&str, &str>,
    cached: bool,
) -> pkl_runtime::evaluation::frontend::ModuleOutputResult {
    let primary_text = modules[primary_uri];
    let inferred = primary_uri
        .rsplit('/')
        .next()
        .unwrap_or("main.pkl")
        .trim_end_matches(".pkl");
    let options = ModuleEvalOptions::default();
    evaluate_module_output_with_hosts(
        LosslessModuleSourceInput {
            text: SourceText::from(primary_text),
            uri: PklString::from(primary_uri),
            inferred_name: PklString::from(inferred),
        },
        &options,
        |request: &ModuleRequest| -> ModuleCompletion {
            if let Some(c) = default_completion(request) {
                return c;
            }
            match &request.kind {
                ModuleRequestKind::SelectKey { id, uri, .. } => {
                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::Host,
                        handle: "virtual".into(),
                    }))
                }
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(cached))
                }
                ModuleRequestKind::QueryFragmentPaths { .. } => {
                    ModuleCompletion::FragmentPaths(Ok(false))
                }
                ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                    id: *resolved_key_id,
                    key: key.clone(),
                    handle: "".into(),
                    module: ResolvedModule {
                        original_uri: key.uri.clone(),
                        resolved_uri: key.uri.clone(),
                        source_uri: key.uri.clone(),
                    },
                })),
                ModuleRequestKind::LoadSource { resolved, .. } => {
                    let uri = resolved.original_uri.as_str().unwrap_or("");
                    if let Some(src) = modules.get(uri) {
                        let name = uri
                            .rsplit('/')
                            .next()
                            .unwrap_or("mod.pkl")
                            .trim_end_matches(".pkl");
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: SourceText::from(*src),
                            inferred_name: PklString::from(name),
                        }))
                    } else {
                        ModuleCompletion::Source(Err(ModuleHostError::NotFound))
                    }
                }
                _ => ModuleCompletion::ImportChecked(Err(ModuleHostError::Unsupported(
                    "unsupported virtual request".into(),
                ))),
            }
        },
        |_| None,
    )
}

#[test]
fn module_instance_equality_skips_hidden_output_and_type_definitions() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/UriLike.pkl",
        r#"module UriLike
import "UriLike.pkl"
typealias SafeStr = String(!isEmpty)
class Helper { x: Int = 1 }
scheme: String?
host: String?
path: SafeStr = "/"
fixed segments: List<String> = path.split("/")
hidden isAbs: Boolean = scheme != null
function make(s: String, h: String): UriLike =
  let (auth = new Dynamic { host = h })
    new UriLike {
      when (s != "") {
        scheme = s
      }
      ...auth
      path = "/a/b"
    }
"#,
    );
    mods.insert(
        "file:///virtual/main.pkl",
        r#"import "UriLike.pkl"
local a = UriLike.make("https", "example.com")
local b = new UriLike {
  scheme = "https"
  host = "example.com"
  path = "/a/b"
}
local c = new UriLike {
  scheme = "http"
  host = "example.com"
  path = "/a/b"
}
eq = a == b
neq = a == c
inSet = Set(a, b, c).length
inMap = Map(a, "hit")[b]
"#,
    );

    let res = eval_virtual_modules("file:///virtual/main.pkl", &mods, true);
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8(res.stdout).unwrap();
    assert_eq!(out, "eq = true\nneq = false\ninSet = 2\ninMap = \"hit\"\n");
}

#[test]
fn mutually_cyclic_module_type_imports_succeed_like_pkl_tar() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/tar.pkl",
        r#"module pkl.tar.tar
import "File.pkl"
typealias FileSystem = Mapping<String, File>
class Pax {
  globalHeaderFields: File.PaxFields
  tag: String = "pax"
}
local const function toTar(f: File): File = f
res = new Pax {
  globalHeaderFields {
    ["k"] = "v"
  }
}
"#,
    );
    mods.insert(
        "file:///virtual/File.pkl",
        r#"abstract module pkl.tar.File
import "File.pkl"
import "tar.pkl"
typealias PaxFields = Mapping<String, String>
function makeHeader(path: String, pax: tar.Pax?): String = "\(path):\(pax?.tag ?? "none")"
"#,
    );

    let res = eval_virtual_modules("file:///virtual/tar.pkl", &mods, true);
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8(res.stdout).unwrap();
    assert!(out.contains("tag = \"pax\""), "unexpected output:\n{out}");
    assert!(out.contains("[\"k\"] = \"v\""), "unexpected output:\n{out}");
}

#[test]
fn expression_access_to_inflight_cyclic_import_during_class_annotation_succeeds() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/ann_a.pkl",
        r#"import "ann_b.pkl"
class MyAnn extends Annotation { tag: String }
class Box {
  @MyAnn { tag = ann_b.TAG }
  x: Int = 1
}
function useB(b: ann_b): Int = 1
res = new Box {}
"#,
    );
    mods.insert(
        "file:///virtual/ann_b.pkl",
        r#"import "ann_a.pkl"
const TAG = "hello"
function useBox(b: ann_a.Box): Int = 2
"#,
    );

    let res = eval_virtual_modules("file:///virtual/ann_a.pkl", &mods, true);
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8(res.stdout).unwrap();
    assert_eq!(out, "res {\n  x = 1\n}\n");
}

#[test]
fn genuine_cross_module_typealias_cycle_still_fails() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/ta_a.pkl",
        r#"import "ta_b.pkl"
typealias X = ta_b.Y
v: X = 1
"#,
    );
    mods.insert(
        "file:///virtual/ta_b.pkl",
        r#"import "ta_a.pkl"
typealias Y = ta_a.X
"#,
    );

    let res = eval_virtual_modules("file:///virtual/ta_a.pkl", &mods, true);
    assert_ne!(res.exit_code, 0);
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(
        err.contains("Type alias definitions must not be cyclic."),
        "unexpected stderr:\n{err}"
    );
}

#[test]
fn genuine_cross_module_property_cycle_still_overflows() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/cyc_a.pkl",
        r#"import "cyc_b.pkl"
x = cyc_b.y
"#,
    );
    mods.insert(
        "file:///virtual/cyc_b.pkl",
        r#"import "cyc_a.pkl"
y = cyc_a.x
"#,
    );

    let res = eval_virtual_modules("file:///virtual/cyc_a.pkl", &mods, true);
    assert_ne!(res.exit_code, 0);
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(
        err.contains("A stack overflow occurred."),
        "unexpected stderr:\n{err}"
    );
}

#[test]
fn uncached_cyclic_module_import_does_not_bypass_cache_policy() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/tar.pkl",
        r#"import "File.pkl"
class Pax { globalHeaderFields: File.PaxFields }
local function toTar(f: File): File = f
res = new Pax { globalHeaderFields {} }
"#,
    );
    mods.insert(
        "file:///virtual/File.pkl",
        r#"import "tar.pkl"
typealias PaxFields = Mapping<String, String>
function makeHeader(pax: tar.Pax?): Int = 0
"#,
    );

    // When ModuleKey.isCached() == false, Java ModuleCache reloads and overflows
    // rather than short-circuiting through the in-flight cached module instance.
    let res = eval_virtual_modules("file:///virtual/tar.pkl", &mods, false);
    assert_ne!(res.exit_code, 0);
}

#[test]
fn typed_with_default_to_dynamic_and_back_to_typed_succeeds() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/Schema.pkl",
        r#"open module Schema
import "Schema.pkl"
type: String?
default: Any?
hidden extensions: Mapping<String, Any>?
class PropertySchema extends Schema {
  xmlName: String?
}
"#,
    );
    mods.insert(
        "file:///virtual/main.pkl",
        r#"import "Schema.pkl"

// Force Dynamic.prototype.default first so Dynamic's prototype has its
// `default: (unknown) -> Any` member installed before Typed.toDynamic() runs.
local warmDynamicDefault = new Dynamic {}.default

local sNull = new Schema { type = "string" }
local dNull = sNull.toDynamic()
local pNull = dNull.toTyped(Schema.PropertySchema)

local sVal = new Schema { type = "integer"; default = 42 }
local dVal = sVal.toDynamic()
local pVal = dVal.toTyped(Schema.PropertySchema)
local dAmended = (dVal) { extra = 1 }

pNullMap = pNull.toMap()
pValDefault = pVal.default
dNullDefault = dNull.default
dAmendedDefault = dAmended.default
dValRenderedMap = dVal.toMap()
hasDefault = dVal.hasProperty("default")
"#,
    );

    let res = eval_virtual_modules("file:///virtual/main.pkl", &mods, true);
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8(res.stdout).unwrap();
    assert!(
        out.contains(r#"pNullMap = Map("type", "string", "default", null, "xmlName", null)"#),
        "unexpected stdout:\n{out}"
    );
    assert!(
        out.contains("pValDefault = 42\n"),
        "unexpected stdout:\n{out}"
    );
    assert!(
        out.contains("dNullDefault = null\n"),
        "unexpected stdout:\n{out}"
    );
    assert!(
        out.contains("dAmendedDefault = 42\n"),
        "unexpected stdout:\n{out}"
    );
    // `default` stays hidden on Dynamic itself for toMap(), matching Java 0.32.0.
    assert!(
        out.contains(r#"dValRenderedMap = Map("type", "integer")"#),
        "unexpected stdout:\n{out}"
    );
    assert!(
        out.contains("hasDefault = true\n"),
        "unexpected stdout:\n{out}"
    );
}

#[test]
fn dynamic_default_typecheck_matches_constant_vs_nonconstant_and_collection_rules() {
    // Non-constant expressions on VmDynamic skip Dynamic.default typecheck
    // (`TypeCheckedPropertyNode.eval(VirtualFrame, VmDynamic)`).
    let mut pass_mods = BTreeMap::new();
    pass_mods.insert(
        "file:///virtual/main.pkl",
        r#"local vNull: Any? = null
local vInt = 40 + 2
local dNull = new Dynamic { default = vNull }
local dInt = new Dynamic { default = vInt }
local dConcat = new Dynamic { default = "a" + "b" }
local dNot = new Dynamic { default = !false }
class Holder { default: Any? }
resNull = dNull.default
resInt = dInt.default
resConcat = dConcat.default
resNot = dNot.default
toTypedVal = dInt.toTyped(Holder).default
"#,
    );
    let pass = eval_virtual_modules("file:///virtual/main.pkl", &pass_mods, true);
    assert_eq!(
        pass.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&pass.stderr)
    );
    assert_eq!(
        String::from_utf8(pass.stdout).unwrap(),
        "resNull = null\nresInt = 42\nresConcat = \"ab\"\nresNot = true\ntoTypedVal = 42\n"
    );

    // Constant members on Dynamic (`VmUtils.doReadMember`) and non-constant
    // members on Listing/Mapping (`!owner.isDynamic()`) still typecheck `default`.
    for (label, source, expected_fragment) in [
        (
            "const_dynamic_null",
            "x = new Dynamic { default = null }\nres = x.default",
            "Expected value of type `Function1`, but got `null`.",
        ),
        (
            "const_dynamic_int",
            "x = new Dynamic { default = 42 }\nres = x.default",
            "Expected value of type `Function1`, but got type `Int`.",
        ),
        (
            "map_to_dynamic_null",
            "d = Map(\"default\", null).toDynamic()\nres = d.default",
            "Expected value of type `Function1`, but got `null`.",
        ),
        (
            "nonconst_listing_null",
            "local v: Any? = null\nx = new Listing { default = v }\nres = x.default",
            "Expected value of type `Function1`, but got `null`.",
        ),
        (
            "nonconst_mapping_null",
            "local v: Any? = null\nx = new Mapping { default = v }\nres = x.default",
            "Expected value of type `Function1`, but got `null`.",
        ),
    ] {
        let mut fail_mods = BTreeMap::new();
        fail_mods.insert("file:///virtual/main.pkl", source);
        let fail = eval_virtual_modules("file:///virtual/main.pkl", &fail_mods, true);
        assert_ne!(
            fail.exit_code,
            0,
            "case `{label}` unexpectedly passed with stdout: {}",
            String::from_utf8_lossy(&fail.stdout)
        );
        let err = String::from_utf8_lossy(&fail.stderr);
        assert!(
            err.contains(expected_fragment),
            "case `{label}` missing `{expected_fragment}` in stderr:\n{err}"
        );
    }
}

#[test]
fn to_typed_open_module_equals_new_module_instance_and_amended_forms() {
    let mut mods = BTreeMap::new();
    mods.insert(
        "file:///virtual/JsonSchemaLike.pkl",
        r#"open module JsonSchemaLike
import "JsonSchemaLike.pkl"
hidden $$baseSchema: JsonSchemaLike?
$ref: String?
type: String?
description: String?
_inline_: Mapping<String, JsonSchemaLike>?
"#,
    );
    mods.insert(
        "file:///virtual/OtherOpenMod.pkl",
        r#"open module OtherOpenMod
type: String?
"#,
    );
    mods.insert(
        "file:///virtual/main.pkl",
        r#"import "JsonSchemaLike.pkl"
import "OtherOpenMod.pkl"

local target = new JsonSchemaLike {
  type = "array"
  _inline_ {
    ["bizzy"] {
      type = "string"
    }
  }
}

local fromMap = (
  (new JsonSchemaLike { $ref = null; $$baseSchema = target }.toMap()
    + target.toMap().filter((_, v) -> v != null))
    .toTyped(JsonSchemaLike.getClass())
) {
  _inline_ { ["__ref_orig__"] = target }
}

local stripped = (fromMap) {
  local newInline = super._inline_.toMap().remove("__ref_orig__").toMapping()
  _inline_ = if (newInline.isEmpty) null else newInline
}

local fromDynamic = new Dynamic {
  type = "array"
  _inline_ = target._inline_
}.toTyped(JsonSchemaLike.getClass())

local emptyFromMap = Map().toTyped(JsonSchemaLike.getClass())
local differentValue = new JsonSchemaLike { type = "object" }
local differentModule = Map("type", "array").toTyped(OtherOpenMod.getClass())

eqStripped = stripped == target
eqDynamic = fromDynamic == target
eqModuleObj = emptyFromMap == JsonSchemaLike
neqValue = stripped == differentValue
neqModule = fromDynamic == differentModule
isModule = fromMap is Module
inSetCount = Set(stripped, target, fromDynamic).length
mapLookup = Map(target, "matched")[stripped]
"#,
    );

    let res = eval_virtual_modules("file:///virtual/main.pkl", &mods, true);
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        "eqStripped = true\neqDynamic = true\neqModuleObj = true\nneqValue = false\nneqModule = false\nisModule = true\ninSetCount = 1\nmapLookup = \"matched\"\n"
    );

    // `reflect.Module(Map().toTyped(OpenMod.getClass()))` must still reject
    // non-module objects (`VmTyped.isModuleObject() == false` in Java 0.32.0).
    let mut reflect_mods = BTreeMap::new();
    reflect_mods.insert(
        "file:///virtual/OpenMod.pkl",
        "open module OpenMod\nx: Int = 1\n",
    );
    reflect_mods.insert(
        "file:///virtual/main.pkl",
        r#"import "pkl:reflect"
import "OpenMod.pkl"
a = Map("x", 1).toTyped(OpenMod.getClass())
r = reflect.Module(a).name
"#,
    );
    let reflect_res = eval_virtual_modules("file:///virtual/main.pkl", &reflect_mods, true);
    assert_ne!(reflect_res.exit_code, 0);
    let err = String::from_utf8_lossy(&reflect_res.stderr);
    assert!(
        err.contains("Expected a module as argument, but got an object that amends a module."),
        "unexpected stderr:\n{err}"
    );
}
