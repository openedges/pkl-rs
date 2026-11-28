//! Wasm-pure `pkl:Command` specification model, reflection decoder, CLI option/argument
//! parser, help formatter, and execution module synthesizer matching Apple Pkl 0.32.0
//! `CommandSpec`, `CommandSpecParser`, and `CliCommandRunner`.

use crate::pkl_binary_decode::DecodedValue;
use std::collections::{BTreeMap, BTreeSet};

/// Completion candidate hint for a `pkl:Command` flag or argument.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandCompletionCandidates {
    Path,
    Fixed(Vec<String>),
}

/// Collection wrapper kind for multiple-valued command options.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCollectionKind {
    Listing,
    List,
    Set,
}

/// Map/mapping wrapper kind for key=value command options.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandMapKind {
    Mapping,
    Map,
}

/// Built-in value parser kind resolved from a `pkl:Command` options property type.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandValueKind {
    String,
    Char,
    Boolean,
    Number,
    Float,
    Int {
        min: i64,
        max: i64,
        metavar: &'static str,
    },
    StringChoices(Vec<String>),
    StringLiteral(String),
    Collection {
        kind: CommandCollectionKind,
        elem: Box<CommandValueKind>,
    },
    MapOrMapping {
        kind: CommandMapKind,
        key: Box<CommandValueKind>,
        value: Box<CommandValueKind>,
    },
    Pair {
        first: Box<CommandValueKind>,
        second: Box<CommandValueKind>,
    },
    /// Non-built-in type handled via custom `convert` and/or `transformAll`.
    Custom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagSpec {
    pub name: String,
    pub short_name: Option<String>,
    pub help_text: Option<String>,
    pub required: bool,
    pub metavar: String,
    pub hidden: bool,
    pub default_display: Option<String>,
    pub multiple: bool,
    pub value_kind: CommandValueKind,
    pub has_convert: bool,
    pub has_transform_all: bool,
    pub completion_candidates: Option<CommandCompletionCandidates>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BooleanFlagSpec {
    pub name: String,
    pub short_name: Option<String>,
    pub help_text: Option<String>,
    pub hidden: bool,
    pub default_value: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountedFlagSpec {
    pub name: String,
    pub short_name: Option<String>,
    pub help_text: Option<String>,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentSpec {
    pub name: String,
    pub help_text: Option<String>,
    pub repeated: bool,
    pub value_kind: CommandValueKind,
    pub has_convert: bool,
    pub has_transform_all: bool,
    pub completion_candidates: Option<CommandCompletionCandidates>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandOptionSpec {
    Flag(FlagSpec),
    BooleanFlag(BooleanFlagSpec),
    CountedFlag(CountedFlagSpec),
    Argument(ArgumentSpec),
}

impl CommandOptionSpec {
    pub fn name(&self) -> &str {
        match self {
            Self::Flag(f) => &f.name,
            Self::BooleanFlag(f) => &f.name,
            Self::CountedFlag(f) => &f.name,
            Self::Argument(a) => &a.name,
        }
    }
}

/// Complete parsed specification of a `pkl:Command` module/class and its subcommands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub module_uri: String,
    /// Accessor path from root module to this command instance (empty for root module,
    /// e.g. `[0, 1]` for `command.subcommands[0].command.subcommands[1]`).
    pub subcommand_path: Vec<usize>,
    pub name: String,
    pub description: Option<String>,
    pub hidden: bool,
    pub no_op: bool,
    pub options_class_name: String,
    pub options: Vec<CommandOptionSpec>,
    pub subcommands: Vec<CommandSpec>,
}

/// Parsed runtime value for a CLI flag or argument before Pkl synthesis.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ParsedOptionValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Pair(Box<ParsedOptionValue>, Box<ParsedOptionValue>),
    List(Vec<ParsedOptionValue>),
    Set(Vec<ParsedOptionValue>),
    Listing(Vec<ParsedOptionValue>),
    Map(Vec<(ParsedOptionValue, ParsedOptionValue)>),
    Mapping(Vec<(ParsedOptionValue, ParsedOptionValue)>),
    /// Raw strings deferred to Pkl `convert` and/or `transformAll` inside the synthesized module.
    RawCustom {
        raw_values: Vec<String>,
        has_convert: bool,
        has_transform_all: bool,
        multiple: bool,
        value_kind: CommandValueKind,
    },
}

/// One invoked command level in a root -> subcommand chain.
#[derive(Clone, Debug)]
pub struct InvokedCommandLevel {
    pub spec: CommandSpec,
    /// Parsed option/argument values keyed by property name (`None` = omitted optional).
    pub values: BTreeMap<String, ParsedOptionValue>,
}

/// Outcome of parsing CLI args against a [`CommandSpec`].
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum CommandParseOutcome {
    Execute(Vec<InvokedCommandLevel>),
    PrintHelp { text: String, exit_code: i32 },
    ShellCompletion { shell: String },
    UsageError { text: String },
}

/// Pure-Pkl reflection expression that inspects a `pkl:Command` module and returns a
/// structured `Mapping` describing the command, its options class, and all subcommands.
///
/// Evaluated via `evaluate_expression_pkl_binary` against the root command module.
pub const COMMAND_SPEC_REFLECT_EXPR: &str = r#"
let (reflect = import("pkl:reflect"))
let (cmdMod = import("pkl:Command"))
let (cmdClass = reflect.Class(cmdMod.getClass()))
let (baseFlagClass = reflect.Class(cmdMod.BaseFlag))
let (flagClass = reflect.Class(cmdMod.Flag))
let (boolFlagClass = reflect.Class(cmdMod.BooleanFlag))
let (countedFlagClass = reflect.Class(cmdMod.CountedFlag))
let (argClass = reflect.Class(cmdMod.Argument))
let (encodeType = null)
let (fnEncodeType = (self, ty) ->
  if (ty is reflect.NullableType) new Mapping {
    ["kind"] = "nullable"
    ["member"] = self.apply(self, ty.member)
  }
  else if (ty is reflect.UnionType) new Mapping {
    ["kind"] = "union"
    ["members"] = ty.members.map((m) -> self.apply(self, m))
  }
  else if (ty is reflect.StringLiteralType) new Mapping {
    ["kind"] = "stringLiteral"
    ["value"] = ty.value
  }
  else if (ty is reflect.DeclaredType) (
    let (ref = ty.referent)
    if (ref is reflect.TypeAlias) new Mapping {
      ["kind"] = "typeAlias"
      ["name"] = ref.name
      ["moduleUri"] = ref.moduleUri
      ["referent"] = self.apply(self, ref.referent)
      ["typeArguments"] = ty.typeArguments.map((a) -> self.apply(self, a))
    } else new Mapping {
      ["kind"] = "class"
      ["name"] = ref.name
      ["qualifiedName"] = if (ref.moduleUri == "pkl:base") ref.name else "\(ref.moduleUri)#\(ref.name)"
      ["moduleUri"] = ref.moduleUri
      ["isAbstract"] = ref.modifiers.contains("abstract")
      ["typeArguments"] = ty.typeArguments.map((a) -> self.apply(self, a))
    }
  )
  else if (ty is reflect.ModuleType) new Mapping { ["kind"] = "module" }
  else if (ty is reflect.UnknownType) new Mapping { ["kind"] = "unknown" }
  else if (ty is reflect.NothingType) new Mapping { ["kind"] = "nothing" }
  else new Mapping { ["kind"] = "other" }
)
let (fnCollectProps = (self, cls, seen) ->
  if (cls == null) List()
  else (
    let (own = cls.properties.toList().filter((p) ->
      !seen.contains(p.first) &&
      !p.second.modifiers.contains("external") &&
      !p.second.modifiers.contains("abstract") &&
      !p.second.modifiers.contains("fixed") &&
      !p.second.modifiers.contains("const")
    ).map((p) ->
      let (prop = p.second)
      let (annots = prop.allAnnotations.map((a) ->
        let (ac = reflect.Class(a.getClass()))
        if (ac == flagClass) new Mapping {
          ["kind"] = "Flag"
          ["shortName"] = a.shortName
          ["hide"] = a.hide
          ["metavar"] = a.metavar
          ["multiple"] = a.multiple
          ["hasConvert"] = a.convert != null
          ["hasTransformAll"] = a.transformAll != null
          ["completionCandidates"] =
            if (a.completionCandidates == null) null
            else if (a.completionCandidates == "paths") "paths"
            else a.completionCandidates.toList()
        }
        else if (ac == boolFlagClass) new Mapping {
          ["kind"] = "BooleanFlag"
          ["shortName"] = a.shortName
          ["hide"] = a.hide
        }
        else if (ac == countedFlagClass) new Mapping {
          ["kind"] = "CountedFlag"
          ["shortName"] = a.shortName
          ["hide"] = a.hide
        }
        else if (ac == argClass) new Mapping {
          ["kind"] = "Argument"
          ["multiple"] = a.multiple
          ["hasConvert"] = a.convert != null
          ["hasTransformAll"] = a.transformAll != null
          ["completionCandidates"] =
            if (a.completionCandidates == null) null
            else if (a.completionCandidates == "paths") "paths"
            else a.completionCandidates.toList()
        }
        else new Mapping { ["kind"] = "Other" }
      ))
      new Mapping {
        ["name"] = prop.name
        ["docComment"] = prop.docComment
        ["type"] = fnEncodeType.apply(fnEncodeType, prop.type)
        ["annotations"] = annots
      }
    ))
    let (nextSeen = seen + cls.properties.keys)
    own + self.apply(self, cls.superclass, nextSeen)
  )
)
let (fnInspectCmd = (self, cmd, path) ->
  let (rc = reflect.Class(cmd.getClass()))
  let (isCmd = rc.isSubclassOf(cmdClass))
  if (!isCmd) new Mapping { ["isCommand"] = false; ["actualClass"] = rc.name }
  else (
    let (info = cmd.command)
    let (optProp = rc.allProperties["options"])
    let (optType = optProp.type)
    let (optClassRef =
      if (optType is reflect.DeclaredType && optType.referent is reflect.Class) optType.referent
      else null
    )
    let (props =
      if (optClassRef == null || optClassRef.name == "Typed" && optClassRef.moduleUri == "pkl:base") List()
      else fnCollectProps.apply(fnCollectProps, optClassRef, Set())
    )
    let (subs = info.subcommands.toList().mapIndexed((idx, sub) ->
      self.apply(self, sub, path + List(idx))
    ))
    new Mapping {
      ["isCommand"] = true
      ["moduleUri"] = rc.moduleUri
      ["path"] = path
      ["name"] = info.name
      ["description"] = info.description
      ["hide"] = info.hide
      ["noOp"] = info.noOp
      ["optionsTypeKind"] = if (optClassRef != null) "class" else "other"
      ["optionsClassName"] = if (optClassRef != null) optClassRef.name else ""
      ["optionsClassModuleUri"] = if (optClassRef != null) optClassRef.moduleUri else ""
      ["optionsClassAbstract"] = if (optClassRef != null) optClassRef.modifiers.contains("abstract") else false
      ["properties"] = props
      ["subcommands"] = subs
    }
  )
)
fnInspectCmd.apply(fnInspectCmd, module, List())
"#;

/// Scans Pkl source for constant primitive defaults & explicit property bodies in a class,
/// and detects illegal assignment/amendment of `options` or `parent` in a `pkl:Command` module.
#[derive(Clone, Debug, Default)]
pub struct SourceCommandMetadata {
    pub assigned_options: bool,
    pub assigned_parent: bool,
    /// Map from `class_name -> prop_name -> DefaultKind`.
    pub prop_defaults: BTreeMap<String, BTreeMap<String, PropDefaultInfo>>,
    /// Map from `class_name -> prop_name -> (has_type_annotation, union_default_literal)`.
    pub prop_type_defaults: BTreeMap<String, BTreeMap<String, Option<String>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropDefaultInfo {
    None,
    Constant(String),
    Complex,
}

/// Inspects a command module's source AST to detect defaults without eagerly evaluating
/// complex default expressions (mirroring `CommandSpecParser.getDefaultValue`).
pub fn extract_source_command_metadata(source: &str) -> SourceCommandMetadata {
    let mut meta = SourceCommandMetadata::default();
    let Ok(parsed) = crate::parser::parse_module(source) else {
        return meta;
    };
    for member in &parsed.members {
        let name = member.name.as_str().unwrap_or("");
        if name == "options" && !matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized)
        {
            meta.assigned_options = true;
        }
        if name == "parent" && !matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
            meta.assigned_parent = true;
        }
        if let crate::syntax::ExprKind::Class {
            members: class_members,
            ..
        } = &member.expr.kind
        {
            let cname = name.to_string();
            let mut defaults = BTreeMap::new();
            let mut type_defaults = BTreeMap::new();
            for prop in class_members {
                let pname = prop.name.as_str().unwrap_or("").to_string();
                let def = match &prop.expr.kind {
                    crate::syntax::ExprKind::Uninitialized => PropDefaultInfo::None,
                    crate::syntax::ExprKind::Int(v) => PropDefaultInfo::Constant(v.to_string()),
                    crate::syntax::ExprKind::Float(v) => {
                        PropDefaultInfo::Constant(crate::numeric::double_to_string(*v))
                    }
                    crate::syntax::ExprKind::Bool(b) => PropDefaultInfo::Constant(b.to_string()),
                    crate::syntax::ExprKind::String(s) => {
                        PropDefaultInfo::Constant(s.to_java_utf8())
                    }
                    _ => PropDefaultInfo::Complex,
                };
                defaults.insert(pname.clone(), def);
                let union_def = prop.ty.as_ref().and_then(extract_union_default_literal);
                type_defaults.insert(pname, union_def);
            }
            meta.prop_defaults.insert(cname.clone(), defaults);
            meta.prop_type_defaults.insert(cname, type_defaults);
        }
    }
    meta
}

fn extract_union_default_literal(ty: &crate::syntax::Type) -> Option<String> {
    match &ty.kind {
        crate::syntax::TypeKind::Union {
            types,
            default: Some(idx),
        } => types.get(*idx).and_then(extract_union_default_literal),
        crate::syntax::TypeKind::String(s) => Some(s.to_java_utf8()),
        crate::syntax::TypeKind::Constrained { base, .. } => extract_union_default_literal(base),
        _ => None,
    }
}

/// Decodes the `pkl-binary` result of [`COMMAND_SPEC_REFLECT_EXPR`] into a validated [`CommandSpec`].
pub fn decode_command_spec(
    decoded: &DecodedValue,
    source_meta: &SourceCommandMetadata,
    reserved_flags: &BTreeSet<String>,
    reserved_shorts: &BTreeSet<String>,
) -> Result<CommandSpec, String> {
    if source_meta.assigned_options {
        return Err("Commands must not assign or amend property `options`.".to_string());
    }
    if source_meta.assigned_parent {
        return Err("Commands must not assign or amend property `parent`.".to_string());
    }
    decode_command_node(decoded, source_meta, reserved_flags, reserved_shorts)
}

fn map_get<'a>(val: &'a DecodedValue, key: &str) -> Option<&'a DecodedValue> {
    match val {
        DecodedValue::Mapping(entries) | DecodedValue::Map(entries) => entries
            .iter()
            .find(|(k, _)| k.to_java_key_string().as_str() == Some(key))
            .map(|(_, v)| v),
        _ => None,
    }
}

fn as_str(val: Option<&DecodedValue>) -> Option<String> {
    match val? {
        DecodedValue::String(s) => Some(s.to_java_utf8()),
        _ => None,
    }
}

fn as_bool(val: Option<&DecodedValue>) -> Option<bool> {
    match val? {
        DecodedValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn as_list(val: Option<&DecodedValue>) -> &[DecodedValue] {
    match val {
        Some(DecodedValue::List(xs) | DecodedValue::Listing(xs)) => xs,
        _ => &[],
    }
}

fn decode_command_node(
    val: &DecodedValue,
    source_meta: &SourceCommandMetadata,
    reserved_flags: &BTreeSet<String>,
    reserved_shorts: &BTreeSet<String>,
) -> Result<CommandSpec, String> {
    if as_bool(map_get(val, "isCommand")) != Some(true) {
        let actual = as_str(map_get(val, "actualClass")).unwrap_or_else(|| "Module".into());
        return Err(format!(
            "Expected value of type `pkl.Command`, but got type `{actual}`."
        ));
    }

    let module_uri = as_str(map_get(val, "moduleUri")).unwrap_or_default();
    let subcommand_path: Vec<usize> = as_list(map_get(val, "path"))
        .iter()
        .filter_map(|v| match v {
            DecodedValue::Int(i) if *i >= 0 => Some(*i as usize),
            _ => None,
        })
        .collect();
    let name = as_str(map_get(val, "name")).unwrap_or_else(|| "command".into());
    let description = as_str(map_get(val, "description"));
    let hidden = as_bool(map_get(val, "hide")).unwrap_or(false);
    let no_op = as_bool(map_get(val, "noOp")).unwrap_or(false);
    let options_type_kind = as_str(map_get(val, "optionsTypeKind")).unwrap_or_default();
    let options_class_name = as_str(map_get(val, "optionsClassName")).unwrap_or_default();
    let options_class_module = as_str(map_get(val, "optionsClassModuleUri")).unwrap_or_default();
    let options_class_abstract = as_bool(map_get(val, "optionsClassAbstract")).unwrap_or(false);

    if options_type_kind != "class" {
        return Err(
            "Type annotation on `options` property in `pkl:Command` subclass must be a class type."
                .to_string(),
        );
    }
    if options_class_abstract
        && !(options_class_name == "Typed" && options_class_module == "pkl:base")
    {
        return Err(format!(
            "Command options class `{options_class_name}` may not be abstract."
        ));
    }

    let class_defaults = source_meta.prop_defaults.get(&options_class_name);
    let class_type_defaults = source_meta.prop_type_defaults.get(&options_class_name);

    let mut options = Vec::new();
    let mut repeated_arg_name: Option<String> = None;

    for prop_val in as_list(map_get(val, "properties")) {
        let prop_name = as_str(map_get(prop_val, "name")).unwrap_or_default();
        let doc_comment = as_str(map_get(prop_val, "docComment"));
        let ty_val = map_get(prop_val, "type");
        let annots = as_list(map_get(prop_val, "annotations"));

        let mut flag_annot: Option<&DecodedValue> = None;
        let mut arg_annot: Option<&DecodedValue> = None;
        for a in annots {
            match as_str(map_get(a, "kind")).as_deref() {
                Some("Flag" | "BooleanFlag" | "CountedFlag") if flag_annot.is_none() => {
                    flag_annot = Some(a);
                }
                Some("Argument") if arg_annot.is_none() => {
                    arg_annot = Some(a);
                }
                _ => {}
            }
        }

        if flag_annot.is_some() && arg_annot.is_some() {
            return Err(format!(
                "Found both `@Flag` and `@Argument` annotations for options property `{prop_name}`.\n\nOnly one option type may be specified."
            ));
        }

        let explicit_default = class_defaults
            .and_then(|m| m.get(&prop_name))
            .cloned()
            .unwrap_or(PropDefaultInfo::None);
        let type_default = class_type_defaults
            .and_then(|m| m.get(&prop_name))
            .cloned()
            .flatten();

        let (resolved_ty, is_nullable) = unwrap_type(ty_val);

        if let Some(arg) = arg_annot {
            if explicit_default != PropDefaultInfo::None {
                return Err(format!(
                    "Unexpected default value for `@Argument` property `{prop_name}`.\n\nArguments may not specify a default value."
                ));
            }
            let has_convert = as_bool(map_get(arg, "hasConvert")).unwrap_or(false);
            let has_transform_all = as_bool(map_get(arg, "hasTransformAll")).unwrap_or(false);
            let explicit_multiple = as_bool(map_get(arg, "multiple"));
            let (value_kind, _metavar, default_multiple, mut candidates) =
                resolve_option_type(&prop_name, resolved_ty, has_convert, has_transform_all)?;
            let repeated = explicit_multiple.unwrap_or(default_multiple);
            if let Some(explicit_cand) =
                decode_completion_candidates(map_get(arg, "completionCandidates"))
            {
                candidates = Some(explicit_cand);
            }
            if is_nullable && !repeated {
                return Err(format!(
                    "Unexpected nullable type for non-collection `@Argument` property `{prop_name}`.\n\nArguments may not be nullable."
                ));
            }
            if repeated {
                if let Some(ref prev) = repeated_arg_name {
                    return Err(format!(
                        "More than one repeated option annotated with `@Argument` found: `{prev}` and `{prop_name}`.\n\nOnly one repeated argument is permitted per command."
                    ));
                }
                repeated_arg_name = Some(prop_name.clone());
            }
            options.push(CommandOptionSpec::Argument(ArgumentSpec {
                name: prop_name,
                help_text: doc_comment,
                repeated,
                value_kind,
                has_convert,
                has_transform_all,
                completion_candidates: candidates,
            }));
            continue;
        }

        let annot_kind = flag_annot.and_then(|a| as_str(map_get(a, "kind")));
        match annot_kind.as_deref() {
            Some("BooleanFlag") => {
                let a = flag_annot.unwrap();
                let short_name = as_str(map_get(a, "shortName"));
                check_flag_collision(
                    &prop_name,
                    short_name.as_deref(),
                    reserved_flags,
                    reserved_shorts,
                )?;
                if !is_base_class(resolved_ty, "Boolean") {
                    return Err(format!(
                        "Option `{prop_name}` with annotation `@BooleanFlag` has invalid type.\nExpected type: `Boolean`"
                    ));
                }
                let default_value = match explicit_default {
                    PropDefaultInfo::Constant(ref s) if s == "true" => Some(true),
                    PropDefaultInfo::Constant(ref s) if s == "false" => Some(false),
                    _ => None,
                };
                options.push(CommandOptionSpec::BooleanFlag(BooleanFlagSpec {
                    name: prop_name,
                    short_name,
                    help_text: doc_comment,
                    hidden: as_bool(map_get(a, "hide")).unwrap_or(false),
                    default_value,
                }));
            }
            Some("CountedFlag") => {
                let a = flag_annot.unwrap();
                let short_name = as_str(map_get(a, "shortName"));
                check_flag_collision(
                    &prop_name,
                    short_name.as_deref(),
                    reserved_flags,
                    reserved_shorts,
                )?;
                if !is_base_class(resolved_ty, "Int") || is_nullable {
                    return Err(format!(
                        "Option `{prop_name}` with annotation `@CountedFlag` has invalid type.\nExpected type: `Int`"
                    ));
                }
                if explicit_default != PropDefaultInfo::None {
                    return Err(format!(
                        "Unexpected default value for `@Argument` property `{prop_name}`.\n\nArguments may not specify a default value."
                    ));
                }
                options.push(CommandOptionSpec::CountedFlag(CountedFlagSpec {
                    name: prop_name,
                    short_name,
                    help_text: doc_comment,
                    hidden: as_bool(map_get(a, "hide")).unwrap_or(false),
                }));
            }
            _ => {
                let short_name = flag_annot.and_then(|a| as_str(map_get(a, "shortName")));
                let hidden = flag_annot
                    .and_then(|a| as_bool(map_get(a, "hide")))
                    .unwrap_or(false);
                let explicit_metavar = flag_annot.and_then(|a| as_str(map_get(a, "metavar")));
                let explicit_multiple = flag_annot.and_then(|a| as_bool(map_get(a, "multiple")));
                let has_convert = flag_annot
                    .and_then(|a| as_bool(map_get(a, "hasConvert")))
                    .unwrap_or(false);
                let has_transform_all = flag_annot
                    .and_then(|a| as_bool(map_get(a, "hasTransformAll")))
                    .unwrap_or(false);
                check_flag_collision(
                    &prop_name,
                    short_name.as_deref(),
                    reserved_flags,
                    reserved_shorts,
                )?;
                let (value_kind, default_metavar, default_multiple, mut candidates) =
                    resolve_option_type(&prop_name, resolved_ty, has_convert, has_transform_all)?;
                if let Some(explicit_cand) = flag_annot
                    .and_then(|a| decode_completion_candidates(map_get(a, "completionCandidates")))
                {
                    candidates = Some(explicit_cand);
                }
                let multiple = explicit_multiple.unwrap_or(default_multiple);
                let metavar = explicit_metavar.unwrap_or(default_metavar);
                let default_display = match &explicit_default {
                    PropDefaultInfo::Constant(s) => Some(s.clone()),
                    PropDefaultInfo::Complex => None,
                    PropDefaultInfo::None if !is_nullable => type_default.clone(),
                    PropDefaultInfo::None => None,
                };
                let has_any_default = explicit_default != PropDefaultInfo::None
                    || (!is_nullable && type_default.is_some());
                let is_optional = is_nullable || has_any_default;
                options.push(CommandOptionSpec::Flag(FlagSpec {
                    name: prop_name,
                    short_name,
                    help_text: doc_comment,
                    required: !is_optional,
                    metavar,
                    hidden,
                    default_display,
                    multiple,
                    value_kind,
                    has_convert,
                    has_transform_all,
                    completion_candidates: candidates,
                }));
            }
        }
    }

    let mut subcommands = Vec::new();
    let mut sub_names = BTreeSet::new();
    for sub_val in as_list(map_get(val, "subcommands")) {
        let sub = decode_command_node(sub_val, source_meta, reserved_flags, reserved_shorts)?;
        if !sub_names.insert(sub.name.clone()) {
            return Err(format!(
                "Command `{name}` has subcommands with conflicting name \"{}\".\nElements of `command.subcommands` must have unique `name` values.",
                sub.name
            ));
        }
        subcommands.push(sub);
    }

    Ok(CommandSpec {
        module_uri,
        subcommand_path,
        name,
        description,
        hidden,
        no_op,
        options_class_name,
        options,
        subcommands,
    })
}

fn check_flag_collision(
    name: &str,
    short_name: Option<&str>,
    reserved_flags: &BTreeSet<String>,
    reserved_shorts: &BTreeSet<String>,
) -> Result<(), String> {
    if reserved_flags.contains(name) {
        return Err(format!(
            "Flag option `{name}` name collides with a reserved flag name.\n\nOption names must not overlap with built-in options."
        ));
    }
    if let Some(s) = short_name
        && reserved_shorts.contains(s)
    {
        return Err(format!(
            "Flag option `{name}` short name `{s}` collides with a reserved flag short name.\n\nOption short names must not overlap with built-in options."
        ));
    }
    Ok(())
}

fn decode_completion_candidates(val: Option<&DecodedValue>) -> Option<CommandCompletionCandidates> {
    match val? {
        DecodedValue::String(s) if s.as_str() == Some("paths") => {
            Some(CommandCompletionCandidates::Path)
        }
        DecodedValue::List(xs) | DecodedValue::Listing(xs) => {
            let items: Vec<String> = xs
                .iter()
                .filter_map(|x| match x {
                    DecodedValue::String(s) => Some(s.to_java_utf8()),
                    _ => None,
                })
                .collect();
            Some(CommandCompletionCandidates::Fixed(items))
        }
        _ => None,
    }
}

fn unwrap_type(mut ty: Option<&DecodedValue>) -> (Option<&DecodedValue>, bool) {
    let mut is_nullable = false;
    while let Some(node) = ty {
        match as_str(map_get(node, "kind")).as_deref() {
            Some("nullable") => {
                is_nullable = true;
                ty = map_get(node, "member");
            }
            Some("typeAlias") => {
                let name = as_str(map_get(node, "name")).unwrap_or_default();
                let mod_uri = as_str(map_get(node, "moduleUri")).unwrap_or_default();
                if mod_uri == "pkl:base"
                    && matches!(
                        name.as_str(),
                        "Char"
                            | "Int8"
                            | "Int16"
                            | "Int32"
                            | "UInt"
                            | "UInt8"
                            | "UInt16"
                            | "UInt32"
                    )
                {
                    break;
                }
                ty = map_get(node, "referent");
            }
            _ => break,
        }
    }
    (ty, is_nullable)
}

fn is_base_class(ty: Option<&DecodedValue>, class_name: &str) -> bool {
    let Some(node) = ty else {
        return false;
    };
    as_str(map_get(node, "kind")).as_deref() == Some("class")
        && as_str(map_get(node, "moduleUri")).as_deref() == Some("pkl:base")
        && as_str(map_get(node, "name")).as_deref() == Some(class_name)
}

fn resolve_primitive_type(
    ty: Option<&DecodedValue>,
) -> Option<(
    CommandValueKind,
    String,
    Option<CommandCompletionCandidates>,
)> {
    let node = ty?;
    let kind = as_str(map_get(node, "kind"))?;
    match kind.as_str() {
        "typeAlias" => {
            let name = as_str(map_get(node, "name")).unwrap_or_default();
            let mod_uri = as_str(map_get(node, "moduleUri")).unwrap_or_default();
            if mod_uri != "pkl:base" {
                return None;
            }
            match name.as_str() {
                "Char" => Some((CommandValueKind::Char, "char".into(), None)),
                "Int8" => Some((
                    CommandValueKind::Int {
                        min: i8::MIN as i64,
                        max: i8::MAX as i64,
                        metavar: "int8",
                    },
                    "int8".into(),
                    None,
                )),
                "Int16" => Some((
                    CommandValueKind::Int {
                        min: i16::MIN as i64,
                        max: i16::MAX as i64,
                        metavar: "int16",
                    },
                    "int16".into(),
                    None,
                )),
                "Int32" => Some((
                    CommandValueKind::Int {
                        min: i32::MIN as i64,
                        max: i32::MAX as i64,
                        metavar: "int32",
                    },
                    "int32".into(),
                    None,
                )),
                "UInt" => Some((
                    CommandValueKind::Int {
                        min: 0,
                        max: i64::MAX,
                        metavar: "uint",
                    },
                    "uint".into(),
                    None,
                )),
                "UInt8" => Some((
                    CommandValueKind::Int {
                        min: 0,
                        max: u8::MAX as i64,
                        metavar: "uint8",
                    },
                    "uint8".into(),
                    None,
                )),
                "UInt16" => Some((
                    CommandValueKind::Int {
                        min: 0,
                        max: u16::MAX as i64,
                        metavar: "uint16",
                    },
                    "uint16".into(),
                    None,
                )),
                "UInt32" => Some((
                    CommandValueKind::Int {
                        min: 0,
                        max: u32::MAX as i64,
                        metavar: "uint32",
                    },
                    "uint32".into(),
                    None,
                )),
                _ => None,
            }
        }
        "class" => {
            if as_str(map_get(node, "moduleUri")).as_deref() != Some("pkl:base") {
                return None;
            }
            match as_str(map_get(node, "name")).as_deref()? {
                "Number" => Some((CommandValueKind::Number, "number".into(), None)),
                "Float" => Some((CommandValueKind::Float, "float".into(), None)),
                "Int" => Some((
                    CommandValueKind::Int {
                        min: i64::MIN,
                        max: i64::MAX,
                        metavar: "int",
                    },
                    "int".into(),
                    None,
                )),
                "Boolean" => Some((CommandValueKind::Boolean, "[true|false]".into(), None)),
                "String" => Some((CommandValueKind::String, "text".into(), None)),
                _ => None,
            }
        }
        "stringLiteral" => {
            let lit = as_str(map_get(node, "value")).unwrap_or_default();
            Some((
                CommandValueKind::StringLiteral(lit.clone()),
                format!("[{lit}]"),
                Some(CommandCompletionCandidates::Fixed(vec![lit])),
            ))
        }
        "union" => {
            let members = as_list(map_get(node, "members"));
            let mut choices = Vec::new();
            for m in members {
                if as_str(map_get(m, "kind")).as_deref() == Some("stringLiteral") {
                    if let Some(s) = as_str(map_get(m, "value")) {
                        choices.push(s);
                    }
                } else {
                    return None;
                }
            }
            if choices.is_empty() {
                return None;
            }
            choices.sort();
            choices.dedup();
            let metavar = format!("[{}]", choices.join(", "));
            Some((
                CommandValueKind::StringChoices(choices.clone()),
                metavar,
                Some(CommandCompletionCandidates::Fixed(choices)),
            ))
        }
        _ => None,
    }
}

fn resolve_option_type(
    prop_name: &str,
    ty: Option<&DecodedValue>,
    has_convert: bool,
    has_transform_all: bool,
) -> Result<
    (
        CommandValueKind,
        String,
        bool,
        Option<CommandCompletionCandidates>,
    ),
    String,
> {
    if let Some((kind, metavar, candidates)) = resolve_primitive_type(ty) {
        return Ok((kind, metavar, false, candidates));
    }
    if let Some(node) = ty
        && as_str(map_get(node, "kind")).as_deref() == Some("class")
        && as_str(map_get(node, "moduleUri")).as_deref() == Some("pkl:base")
    {
        let cname = as_str(map_get(node, "name")).unwrap_or_default();
        let targs = as_list(map_get(node, "typeArguments"));
        match cname.as_str() {
            "Listing" | "List" | "Set" => {
                let col_kind = match cname.as_str() {
                    "Listing" => CommandCollectionKind::Listing,
                    "List" => CommandCollectionKind::List,
                    _ => CommandCollectionKind::Set,
                };
                let arg0 = targs.first();
                let (inner_ty, _) = unwrap_type(arg0);
                if let Some((elem_kind, elem_meta, cand)) = resolve_primitive_type(inner_ty) {
                    return Ok((
                        CommandValueKind::Collection {
                            kind: col_kind,
                            elem: Box::new(elem_kind),
                        },
                        elem_meta,
                        true,
                        cand,
                    ));
                }
                if has_convert || has_transform_all {
                    return Ok((
                        CommandValueKind::Collection {
                            kind: col_kind,
                            elem: Box::new(CommandValueKind::Custom),
                        },
                        "value".into(),
                        true,
                        None,
                    ));
                }
                return Err(format!(
                    "Command option property `{prop_name}` has unsupported type `{cname}`.\n\n{cname} options must provide one type argument."
                ));
            }
            "Mapping" | "Map" => {
                let map_kind = if cname == "Mapping" {
                    CommandMapKind::Mapping
                } else {
                    CommandMapKind::Map
                };
                let (k_ty, _) = unwrap_type(targs.first());
                let (v_ty, _) = unwrap_type(targs.get(1));
                if let (Some((kk, km, _)), Some((vk, vm, _))) =
                    (resolve_primitive_type(k_ty), resolve_primitive_type(v_ty))
                {
                    return Ok((
                        CommandValueKind::MapOrMapping {
                            kind: map_kind,
                            key: Box::new(kk),
                            value: Box::new(vk),
                        },
                        format!("{km}={vm}"),
                        true,
                        None,
                    ));
                }
                if has_convert || has_transform_all {
                    return Ok((
                        CommandValueKind::MapOrMapping {
                            kind: map_kind,
                            key: Box::new(CommandValueKind::Custom),
                            value: Box::new(CommandValueKind::Custom),
                        },
                        "value".into(),
                        true,
                        None,
                    ));
                }
                return Err(format!(
                    "Command option property `{prop_name}` has unsupported type `{cname}`.\n\n{cname} options must provide two type arguments."
                ));
            }
            "Pair" => {
                let (k_ty, _) = unwrap_type(targs.first());
                let (v_ty, _) = unwrap_type(targs.get(1));
                if let (Some((kk, km, _)), Some((vk, vm, _))) =
                    (resolve_primitive_type(k_ty), resolve_primitive_type(v_ty))
                {
                    return Ok((
                        CommandValueKind::Pair {
                            first: Box::new(kk),
                            second: Box::new(vk),
                        },
                        format!("{km}={vm}"),
                        false,
                        None,
                    ));
                }
                if has_convert || has_transform_all {
                    return Ok((
                        CommandValueKind::Pair {
                            first: Box::new(CommandValueKind::Custom),
                            second: Box::new(CommandValueKind::Custom),
                        },
                        "value".into(),
                        false,
                        None,
                    ));
                }
                return Err(format!(
                    "Command option property `{prop_name}` has unsupported type `Pair`.\n\nPair options must provide two type arguments."
                ));
            }
            _ => {}
        }
    }

    if has_convert || has_transform_all {
        return Ok((CommandValueKind::Custom, "value".into(), false, None));
    }

    Err(format!(
        "Command option property `{prop_name}` has unsupported type.\n\nUse a supported type or define a transformEach and/or transformAll function"
    ))
}

/// Parses a single primitive CLI value according to [`CommandValueKind`].
pub fn parse_single_value(raw: &str, kind: &CommandValueKind) -> Result<ParsedOptionValue, String> {
    match kind {
        CommandValueKind::String | CommandValueKind::Custom => {
            Ok(ParsedOptionValue::String(raw.to_string()))
        }
        CommandValueKind::Char => {
            if raw.encode_utf16().count() != 1 {
                return Err(format!("{raw} is not a valid char"));
            }
            Ok(ParsedOptionValue::String(raw.to_string()))
        }
        CommandValueKind::Boolean => match raw.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" | "yes" | "y" | "on" => Ok(ParsedOptionValue::Bool(true)),
            "false" | "f" | "0" | "no" | "n" | "off" => Ok(ParsedOptionValue::Bool(false)),
            _ => Err(format!("{raw} is not a valid boolean")),
        },
        CommandValueKind::Number => {
            if let Ok(i) = raw.parse::<i64>() {
                Ok(ParsedOptionValue::Int(i))
            } else if let Ok(f) = raw.parse::<f64>() {
                Ok(ParsedOptionValue::Float(f))
            } else {
                Err(format!("{raw} is not a valid number"))
            }
        }
        CommandValueKind::Float => raw
            .parse::<f64>()
            .map(ParsedOptionValue::Float)
            .map_err(|_| format!("{raw} is not a valid float")),
        CommandValueKind::Int { min, max, metavar } => match raw.parse::<i64>() {
            Ok(v) if v >= *min && v <= *max => Ok(ParsedOptionValue::Int(v)),
            _ => Err(format!("{raw} is not a valid {metavar}")),
        },
        CommandValueKind::StringChoices(choices) => {
            if choices.iter().any(|c| c == raw) {
                Ok(ParsedOptionValue::String(raw.to_string()))
            } else {
                Err(format!(
                    "invalid choice: {raw}. (choose from {})",
                    choices.join(", ")
                ))
            }
        }
        CommandValueKind::StringLiteral(expected) => {
            if raw == expected {
                Ok(ParsedOptionValue::String(raw.to_string()))
            } else {
                Err(format!("invalid choice: {raw}. (choose from {expected})"))
            }
        }
        CommandValueKind::Collection { elem, .. } => parse_single_value(raw, elem),
        CommandValueKind::MapOrMapping { key, value, .. }
        | CommandValueKind::Pair {
            first: key,
            second: value,
        } => {
            let Some((k_raw, v_raw)) = raw.split_once('=') else {
                return Err(format!("invalid key=value pair: {raw}"));
            };
            let k = parse_single_value(k_raw, key)?;
            let v = parse_single_value(v_raw, value)?;
            Ok(ParsedOptionValue::Pair(Box::new(k), Box::new(v)))
        }
    }
}

/// Parses CLI arguments against a root [`CommandSpec`] (including recursive subcommands,
/// `--help`/`-h`, and root `shell-completion <bash|zsh|fish>`).
pub fn parse_command_invocation(
    root_spec: &CommandSpec,
    display_root_name: &str,
    args: &[String],
) -> CommandParseOutcome {
    let mut chain = Vec::new();
    let mut path_names = vec![display_root_name.to_string()];
    parse_level(root_spec, &mut path_names, args, true, &mut chain)
}

fn parse_level(
    spec: &CommandSpec,
    path_names: &mut Vec<String>,
    args: &[String],
    is_root: bool,
    chain: &mut Vec<InvokedCommandLevel>,
) -> CommandParseOutcome {
    let full_cmd = path_names.join(" ");
    let mut raw_flag_values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut bool_flag_values: BTreeMap<String, bool> = BTreeMap::new();
    let mut counted_flag_values: BTreeMap<String, i64> = BTreeMap::new();
    let mut positional: Vec<String> = Vec::new();
    let mut subcmd_invoked: Option<(usize, usize)> = None;
    let mut shell_completion_start: Option<usize> = None;

    let mut i = 0usize;
    let mut after_double_dash = false;
    while i < args.len() {
        let arg = &args[i];
        if !after_double_dash && arg == "--" {
            after_double_dash = true;
            i += 1;
            continue;
        }
        if !after_double_dash && (arg == "-h" || arg == "--help") {
            return CommandParseOutcome::PrintHelp {
                text: render_command_help(spec, &full_cmd, is_root),
                exit_code: 0,
            };
        }
        if !after_double_dash && arg.starts_with("--") {
            let body = &arg[2..];
            let (opt_name, attached_val) = match body.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (body, None),
            };
            // Check boolean `--no-<name>`
            if let Some(neg) = opt_name.strip_prefix("no-")
                && spec
                    .options
                    .iter()
                    .any(|o| matches!(o, CommandOptionSpec::BooleanFlag(b) if b.name == neg))
            {
                if attached_val.is_some() {
                    return usage_err(
                        spec,
                        &full_cmd,
                        is_root,
                        &format!("option --{opt_name} does not take a value"),
                    );
                }
                bool_flag_values.insert(neg.to_string(), false);
                i += 1;
                continue;
            }
            if let Some(opt) = spec.options.iter().find(|o| match o {
                CommandOptionSpec::Flag(f) => f.name == opt_name,
                CommandOptionSpec::BooleanFlag(f) => f.name == opt_name,
                CommandOptionSpec::CountedFlag(f) => f.name == opt_name,
                CommandOptionSpec::Argument(_) => false,
            }) {
                match opt {
                    CommandOptionSpec::Flag(f) => {
                        let val = if let Some(v) = attached_val {
                            i += 1;
                            v.to_string()
                        } else if i + 1 < args.len() {
                            let v = args[i + 1].clone();
                            i += 2;
                            v
                        } else {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} requires a value", f.name),
                            );
                        };
                        raw_flag_values.entry(f.name.clone()).or_default().push(val);
                    }
                    CommandOptionSpec::BooleanFlag(f) => {
                        if attached_val.is_some() {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} does not take a value", f.name),
                            );
                        }
                        bool_flag_values.insert(f.name.clone(), true);
                        i += 1;
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        if attached_val.is_some() {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option --{} does not take a value", f.name),
                            );
                        }
                        *counted_flag_values.entry(f.name.clone()).or_insert(0) += 1;
                        i += 1;
                    }
                    CommandOptionSpec::Argument(_) => unreachable!(),
                }
                continue;
            }
            return usage_err(
                spec,
                &full_cmd,
                is_root,
                &format!("no such option --{opt_name}"),
            );
        }
        if !after_double_dash && arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut ci = 0;
            while ci < chars.len() {
                let c = chars[ci].to_string();
                if c == "h" {
                    return CommandParseOutcome::PrintHelp {
                        text: render_command_help(spec, &full_cmd, is_root),
                        exit_code: 0,
                    };
                }
                let Some(opt) = spec.options.iter().find(|o| match o {
                    CommandOptionSpec::Flag(f) => f.short_name.as_deref() == Some(c.as_str()),
                    CommandOptionSpec::BooleanFlag(f) => {
                        f.short_name.as_deref() == Some(c.as_str())
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        f.short_name.as_deref() == Some(c.as_str())
                    }
                    CommandOptionSpec::Argument(_) => false,
                }) else {
                    return usage_err(spec, &full_cmd, is_root, &format!("no such option -{c}"));
                };
                match opt {
                    CommandOptionSpec::Flag(f) => {
                        let rest: String = chars[ci + 1..].iter().collect();
                        let val = if !rest.is_empty() {
                            i += 1;
                            rest
                        } else if i + 1 < args.len() {
                            let v = args[i + 1].clone();
                            i += 2;
                            v
                        } else {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("option -{c} requires a value"),
                            );
                        };
                        raw_flag_values.entry(f.name.clone()).or_default().push(val);
                        break;
                    }
                    CommandOptionSpec::BooleanFlag(f) => {
                        bool_flag_values.insert(f.name.clone(), true);
                        ci += 1;
                        if ci == chars.len() {
                            i += 1;
                        }
                    }
                    CommandOptionSpec::CountedFlag(f) => {
                        *counted_flag_values.entry(f.name.clone()).or_insert(0) += 1;
                        ci += 1;
                        if ci == chars.len() {
                            i += 1;
                        }
                    }
                    CommandOptionSpec::Argument(_) => unreachable!(),
                }
            }
            continue;
        }

        // Check for subcommand match once required positional args are satisfied (or immediately if no args).
        let arg_specs: Vec<&ArgumentSpec> = spec
            .options
            .iter()
            .filter_map(|o| match o {
                CommandOptionSpec::Argument(a) => Some(a),
                _ => None,
            })
            .collect();
        if positional.len() >= arg_specs.iter().filter(|a| !a.repeated).count() {
            if is_root && arg == "shell-completion" {
                shell_completion_start = Some(i + 1);
                break;
            }
            if let Some(sub_idx) = spec.subcommands.iter().position(|s| s.name == *arg) {
                subcmd_invoked = Some((sub_idx, i + 1));
                break;
            }
        }
        positional.push(arg.clone());
        i += 1;
    }

    if let Some(sc_start) = shell_completion_start {
        let sc_args = &args[sc_start..];
        let sc_usage = format!("Usage: {full_cmd} shell-completion [<options>] <shell>\n");
        if sc_args.iter().any(|a| a == "-h" || a == "--help") {
            return CommandParseOutcome::PrintHelp {
                text: format!(
                    "{sc_usage}\nGenerate a completion script for the given shell\n\nOptions:\n  -h, --help  Show this message and exit\n\nArguments:\n  <shell>\n"
                ),
                exit_code: 0,
            };
        }
        let Some(shell) = sc_args.first() else {
            return CommandParseOutcome::UsageError {
                text: format!("{sc_usage}\nError: missing argument <shell>\n"),
            };
        };
        if !matches!(shell.as_str(), "bash" | "zsh" | "fish") {
            return CommandParseOutcome::UsageError {
                text: format!(
                    "{sc_usage}\nError: invalid value for <shell>: invalid choice: {shell}. (choose from bash, zsh, fish)\n"
                ),
            };
        }
        return CommandParseOutcome::ShellCompletion {
            shell: shell.clone(),
        };
    }

    // Build parsed option values for this command level
    let mut values: BTreeMap<String, ParsedOptionValue> = BTreeMap::new();
    for opt in &spec.options {
        match opt {
            CommandOptionSpec::Flag(f) => {
                let raws = raw_flag_values.remove(&f.name).unwrap_or_default();
                if raws.is_empty() {
                    if f.required {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("missing option --{}", f.name),
                        );
                    }
                    continue;
                }
                if f.has_convert || f.has_transform_all {
                    values.insert(
                        f.name.clone(),
                        ParsedOptionValue::RawCustom {
                            raw_values: raws,
                            has_convert: f.has_convert,
                            has_transform_all: f.has_transform_all,
                            multiple: f.multiple,
                            value_kind: f.value_kind.clone(),
                        },
                    );
                } else {
                    match parse_all_values(&raws, &f.value_kind, f.multiple) {
                        Ok(parsed) => {
                            values.insert(f.name.clone(), parsed);
                        }
                        Err(msg) => {
                            return usage_err(
                                spec,
                                &full_cmd,
                                is_root,
                                &format!("invalid value for --{}: {msg}", f.name),
                            );
                        }
                    }
                }
            }
            CommandOptionSpec::BooleanFlag(f) => {
                if let Some(b) = bool_flag_values.remove(&f.name) {
                    values.insert(f.name.clone(), ParsedOptionValue::Bool(b));
                } else if let Some(def) = f.default_value {
                    values.insert(f.name.clone(), ParsedOptionValue::Bool(def));
                }
            }
            CommandOptionSpec::CountedFlag(f) => {
                let count = counted_flag_values.remove(&f.name).unwrap_or(0);
                values.insert(f.name.clone(), ParsedOptionValue::Int(count));
            }
            CommandOptionSpec::Argument(_) => {}
        }
    }

    // Bind positional arguments
    let arg_specs: Vec<&ArgumentSpec> = spec
        .options
        .iter()
        .filter_map(|o| match o {
            CommandOptionSpec::Argument(a) => Some(a),
            _ => None,
        })
        .collect();
    let mut pos_idx = 0usize;
    for (a_idx, a) in arg_specs.iter().enumerate() {
        if a.repeated {
            let remaining_after = arg_specs.len() - a_idx - 1;
            let take_end = positional.len().saturating_sub(remaining_after);
            let slice: Vec<String> = if pos_idx <= take_end {
                positional[pos_idx..take_end].to_vec()
            } else {
                Vec::new()
            };
            pos_idx = take_end;
            if slice.is_empty() {
                continue;
            }
            if a.has_convert || a.has_transform_all {
                values.insert(
                    a.name.clone(),
                    ParsedOptionValue::RawCustom {
                        raw_values: slice,
                        has_convert: a.has_convert,
                        has_transform_all: a.has_transform_all,
                        multiple: true,
                        value_kind: a.value_kind.clone(),
                    },
                );
            } else {
                match parse_all_values(&slice, &a.value_kind, true) {
                    Ok(parsed) => {
                        values.insert(a.name.clone(), parsed);
                    }
                    Err(msg) => {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("invalid value for <{}>: {msg}", a.name),
                        );
                    }
                }
            }
        } else {
            let Some(raw) = positional.get(pos_idx) else {
                return usage_err(
                    spec,
                    &full_cmd,
                    is_root,
                    &format!("missing argument <{}>", a.name),
                );
            };
            pos_idx += 1;
            if a.has_convert || a.has_transform_all {
                values.insert(
                    a.name.clone(),
                    ParsedOptionValue::RawCustom {
                        raw_values: vec![raw.clone()],
                        has_convert: a.has_convert,
                        has_transform_all: a.has_transform_all,
                        multiple: false,
                        value_kind: a.value_kind.clone(),
                    },
                );
            } else {
                match parse_single_value(raw, &a.value_kind) {
                    Ok(parsed) => {
                        values.insert(a.name.clone(), parsed);
                    }
                    Err(msg) => {
                        return usage_err(
                            spec,
                            &full_cmd,
                            is_root,
                            &format!("invalid value for <{}>: {msg}", a.name),
                        );
                    }
                }
            }
        }
    }

    if pos_idx < positional.len() {
        let extra = &positional[pos_idx..];
        let msg = if extra.len() == 1 {
            format!("got unexpected extra argument ({})", extra[0])
        } else {
            format!("got unexpected extra arguments ({})", extra.join(" "))
        };
        return usage_err(spec, &full_cmd, is_root, &msg);
    }

    chain.push(InvokedCommandLevel {
        spec: spec.clone(),
        values,
    });

    if let Some((sub_idx, sub_start)) = subcmd_invoked {
        let sub_spec = &spec.subcommands[sub_idx];
        path_names.push(sub_spec.name.clone());
        return parse_level(sub_spec, path_names, &args[sub_start..], false, chain);
    }

    if !spec.subcommands.is_empty() && spec.no_op {
        return CommandParseOutcome::PrintHelp {
            text: render_command_help(spec, &full_cmd, is_root),
            exit_code: 1,
        };
    }

    CommandParseOutcome::Execute(std::mem::take(chain))
}

fn parse_all_values(
    raws: &[String],
    kind: &CommandValueKind,
    multiple: bool,
) -> Result<ParsedOptionValue, String> {
    let mut parsed_items = Vec::with_capacity(raws.len());
    for raw in raws {
        parsed_items.push(parse_single_value(raw, kind)?);
    }
    if !multiple {
        return Ok(parsed_items.pop().unwrap());
    }
    match kind {
        CommandValueKind::Collection {
            kind: CommandCollectionKind::Listing,
            ..
        } => Ok(ParsedOptionValue::Listing(parsed_items)),
        CommandValueKind::Collection {
            kind: CommandCollectionKind::List,
            ..
        } => Ok(ParsedOptionValue::List(parsed_items)),
        CommandValueKind::Collection {
            kind: CommandCollectionKind::Set,
            ..
        } => Ok(ParsedOptionValue::Set(parsed_items)),
        CommandValueKind::MapOrMapping {
            kind: CommandMapKind::Mapping,
            ..
        } => {
            let pairs = parsed_items
                .into_iter()
                .map(|p| match p {
                    ParsedOptionValue::Pair(k, v) => (*k, *v),
                    other => (ParsedOptionValue::String(String::new()), other),
                })
                .collect();
            Ok(ParsedOptionValue::Mapping(pairs))
        }
        CommandValueKind::MapOrMapping {
            kind: CommandMapKind::Map,
            ..
        } => {
            let pairs = parsed_items
                .into_iter()
                .map(|p| match p {
                    ParsedOptionValue::Pair(k, v) => (*k, *v),
                    other => (ParsedOptionValue::String(String::new()), other),
                })
                .collect();
            Ok(ParsedOptionValue::Map(pairs))
        }
        _ => Ok(parsed_items.pop().unwrap()),
    }
}

fn format_usage_line(spec: &CommandSpec, full_cmd: &str, is_root: bool) -> String {
    let mut line = format!("Usage: {full_cmd} [<options>]");
    for opt in &spec.options {
        if let CommandOptionSpec::Argument(a) = opt {
            if a.repeated {
                line.push_str(&format!(" [<{}>]...", a.name));
            } else {
                line.push_str(&format!(" <{}>", a.name));
            }
        }
    }
    if is_root || !spec.subcommands.is_empty() {
        line.push_str(" <command> [<args>]...");
    }
    line
}

fn usage_err(spec: &CommandSpec, full_cmd: &str, is_root: bool, msg: &str) -> CommandParseOutcome {
    let usage = format_usage_line(spec, full_cmd, is_root);
    CommandParseOutcome::UsageError {
        text: format!("{usage}\n\nError: {msg}\n"),
    }
}

/// Renders Clikt-formatted help text for a `pkl:Command` or subcommand.
pub fn render_command_help(spec: &CommandSpec, full_cmd: &str, is_root: bool) -> String {
    let mut out = format_usage_line(spec, full_cmd, is_root);
    out.push('\n');
    if let Some(ref desc) = spec.description {
        let trimmed = desc.trim();
        if !trimmed.is_empty() {
            out.push('\n');
            out.push_str(&format!("  {trimmed}\n"));
        }
    }

    let mut opt_rows: Vec<(String, String)> = Vec::new();
    for opt in &spec.options {
        match opt {
            CommandOptionSpec::Flag(f) if !f.hidden => {
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{}={}", f.name, f.metavar),
                    None => format!("--{}={}", f.name, f.metavar),
                };
                let mut help = f.help_text.clone().unwrap_or_default().trim().to_string();
                if let Some(ref def) = f.default_display {
                    if !help.is_empty() {
                        help.push(' ');
                    }
                    help.push_str(&format!("(default: {def})"));
                } else if f.required {
                    if !help.is_empty() {
                        help.push(' ');
                    }
                    help.push_str("(required)");
                }
                opt_rows.push((names, help));
            }
            CommandOptionSpec::BooleanFlag(f) if !f.hidden => {
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{} / --no-{}", f.name, f.name),
                    None => format!("--{} / --no-{}", f.name, f.name),
                };
                opt_rows.push((
                    names,
                    f.help_text.clone().unwrap_or_default().trim().to_string(),
                ));
            }
            CommandOptionSpec::CountedFlag(f) if !f.hidden => {
                let names = match &f.short_name {
                    Some(s) => format!("-{s}, --{}", f.name),
                    None => format!("--{}", f.name),
                };
                opt_rows.push((
                    names,
                    f.help_text.clone().unwrap_or_default().trim().to_string(),
                ));
            }
            _ => {}
        }
    }
    opt_rows.push(("-h, --help".into(), "Show this message and exit".into()));

    out.push_str("\nOptions:\n");
    let max_opt = opt_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (left, right) in &opt_rows {
        if right.is_empty() {
            out.push_str(&format!("  {left}\n"));
        } else {
            let pad = " ".repeat(max_opt - left.len() + 2);
            out.push_str(&format!("  {left}{pad}{right}\n"));
        }
    }

    let arg_rows: Vec<(String, String)> = spec
        .options
        .iter()
        .filter_map(|o| match o {
            CommandOptionSpec::Argument(a) => Some((
                format!("<{}>", a.name),
                a.help_text.clone().unwrap_or_default().trim().to_string(),
            )),
            _ => None,
        })
        .collect();
    if !arg_rows.is_empty() {
        out.push_str("\nArguments:\n");
        let max_arg = arg_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
        for (left, right) in &arg_rows {
            if right.is_empty() {
                out.push_str(&format!("  {left}\n"));
            } else {
                let pad = " ".repeat(max_arg - left.len() + 2);
                out.push_str(&format!("  {left}{pad}{right}\n"));
            }
        }
    }

    let mut cmd_rows: Vec<(String, String)> = spec
        .subcommands
        .iter()
        .filter(|s| !s.hidden)
        .map(|s| {
            (
                s.name.clone(),
                s.description.clone().unwrap_or_default().trim().to_string(),
            )
        })
        .collect();
    if is_root {
        cmd_rows.push((
            "shell-completion".into(),
            "Generate a completion script for the given shell".into(),
        ));
    }
    if !cmd_rows.is_empty() {
        out.push_str("\nCommands:\n");
        let max_cmd = cmd_rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
        for (left, right) in &cmd_rows {
            if right.is_empty() {
                out.push_str(&format!("  {left}\n"));
            } else {
                let pad = " ".repeat(max_cmd - left.len() + 2);
                out.push_str(&format!("  {left}{pad}{right}\n"));
            }
        }
    }
    out
}

/// Synthesizes a Pkl amendment module that populates `options` (and `parent` chain) on the
/// invoked command so `output.bytes` and `output.files` evaluate identically to
/// `CommandSpecParser.buildExecutionModule` / `evaluateResult`.
pub fn synthesize_command_eval_module(
    root_module_uri: &str,
    levels: &[InvokedCommandLevel],
    working_dir_uri: &str,
) -> String {
    assert!(!levels.is_empty(), "at least one invoked command level");
    let target = levels.last().unwrap();

    // Helper Pkl prelude for resolving `Command.Import` values returned by `convert`/`transformAll`
    let mut out = String::new();
    if target.spec.subcommand_path.is_empty() {
        out.push_str(&format!("amends {}\n\n", pkl_quote_string(root_module_uri)));
    } else {
        out.push_str("import \"pkl:Command\" as __pklCmd\n");
        out.push_str(&format!(
            "local __rootMod = import({})\n",
            pkl_quote_string(root_module_uri)
        ));
    }

    let has_any_custom = levels.iter().any(|l| {
        l.values
            .values()
            .any(|v| matches!(v, ParsedOptionValue::RawCustom { .. }))
    });
    if has_any_custom {
        out.push_str("import \"pkl:reflect\" as __pklReflect\n");
        out.push_str("import \"pkl:Command\" as __pklCmdMod\n");
        out.push_str(&format!(
            "local __workingDirUri = {}\n",
            pkl_quote_string(working_dir_uri)
        ));
        out.push_str(COMMAND_IMPORT_HELPER_PKL);
    }

    if target.spec.subcommand_path.is_empty() {
        out.push_str("options {\n");
        write_level_options(&mut out, target, "module", "  ");
        out.push_str("}\n");
    } else {
        // Build nested parent chain and amend target subcommand as `output`
        let mut parent_expr = String::new();
        for (idx, level) in levels.iter().enumerate() {
            let cmd_ref = subcommand_ref_expr("__rootMod", &level.spec.subcommand_path);
            let mut body = String::new();
            body.push_str("options {\n");
            write_level_options(&mut body, level, &cmd_ref, "    ");
            body.push_str("  }\n");
            if idx > 0 {
                body.push_str(&format!("  parent = {parent_expr}\n"));
            }
            parent_expr = format!("({cmd_ref}) {{\n  {body}}}");
        }
        out.push_str(&format!("local __executed = {parent_expr}\n"));
        out.push_str("output = __executed.output\n");
    }

    out
}

fn subcommand_ref_expr(root_var: &str, path: &[usize]) -> String {
    let mut s = root_var.to_string();
    for idx in path {
        s.push_str(&format!(".command.subcommands[{idx}]"));
    }
    s
}

fn write_level_options(out: &mut String, level: &InvokedCommandLevel, cmd_ref: &str, indent: &str) {
    for (name, val) in &level.values {
        let ident = pkl_ident(name);
        let expr = render_parsed_value(val, cmd_ref, name);
        out.push_str(&format!("{indent}{ident} = {expr}\n"));
    }
}

fn render_parsed_value(val: &ParsedOptionValue, cmd_ref: &str, prop_name: &str) -> String {
    match val {
        ParsedOptionValue::String(s) => pkl_quote_string(s),
        ParsedOptionValue::Int(i) => i.to_string(),
        ParsedOptionValue::Float(f) => crate::numeric::double_to_string(*f),
        ParsedOptionValue::Bool(b) => b.to_string(),
        ParsedOptionValue::Pair(a, b) => format!(
            "Pair({}, {})",
            render_parsed_value(a, cmd_ref, prop_name),
            render_parsed_value(b, cmd_ref, prop_name)
        ),
        ParsedOptionValue::List(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("List({})", elems.join(", "))
        }
        ParsedOptionValue::Set(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("Set({})", elems.join(", "))
        }
        ParsedOptionValue::Listing(items) => {
            let elems: Vec<String> = items
                .iter()
                .map(|x| render_parsed_value(x, cmd_ref, prop_name))
                .collect();
            format!("new Listing {{ {} }}", elems.join("; "))
        }
        ParsedOptionValue::Map(entries) => {
            let mut args = Vec::with_capacity(entries.len() * 2);
            for (k, v) in entries {
                args.push(render_parsed_value(k, cmd_ref, prop_name));
                args.push(render_parsed_value(v, cmd_ref, prop_name));
            }
            format!("Map({})", args.join(", "))
        }
        ParsedOptionValue::Mapping(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| {
                    format!(
                        "[{}] = {}",
                        render_parsed_value(k, cmd_ref, prop_name),
                        render_parsed_value(v, cmd_ref, prop_name)
                    )
                })
                .collect();
            format!("new Mapping {{ {} }}", parts.join("; "))
        }
        ParsedOptionValue::RawCustom {
            raw_values,
            has_convert,
            has_transform_all,
            multiple,
            value_kind,
        } => {
            let raw_list: Vec<String> = raw_values.iter().map(|s| pkl_quote_string(s)).collect();
            let raw_list_expr = format!("List({})", raw_list.join(", "));
            let prop_q = pkl_quote_string(prop_name);
            let converted_expr = if *has_convert {
                format!(
                    "let (__a = __pklReflect.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> a is __pklCmdMod.Flag || a is __pklCmdMod.Argument)) {raw_list_expr}.map((__raw) -> __resolveCmdImports(__a.convert.apply(__raw)))"
                )
            } else {
                // Parse each using built-in parser when only transformAll is provided
                let parsed: Vec<String> = raw_values
                    .iter()
                    .map(|r| match parse_single_value(r, value_kind) {
                        Ok(pv) => render_parsed_value(&pv, cmd_ref, prop_name),
                        Err(_) => pkl_quote_string(r),
                    })
                    .collect();
                format!("List({})", parsed.join(", "))
            };
            if *has_transform_all {
                format!(
                    "let (__a = __pklReflect.Class({cmd_ref}.getClass()).allProperties[\"options\"].type.referent.allProperties[{prop_q}].allAnnotations.find((a) -> a is __pklCmdMod.Flag || a is __pklCmdMod.Argument)) __resolveCmdImports(__a.transformAll.apply({converted_expr}))"
                )
            } else if *multiple {
                match value_kind {
                    CommandValueKind::Collection {
                        kind: CommandCollectionKind::Listing,
                        ..
                    } => format!("({converted_expr}).toListing()"),
                    CommandValueKind::Collection {
                        kind: CommandCollectionKind::Set,
                        ..
                    } => format!("({converted_expr}).toSet()"),
                    CommandValueKind::MapOrMapping {
                        kind: CommandMapKind::Mapping,
                        ..
                    } => format!(
                        "new Mapping {{ for (__p in {converted_expr}) [__p.first] = __p.second }}"
                    ),
                    CommandValueKind::MapOrMapping {
                        kind: CommandMapKind::Map,
                        ..
                    } => {
                        format!("({converted_expr}).toMap((__p) -> __p.first, (__p) -> __p.second)")
                    }
                    _ => converted_expr,
                }
            } else {
                format!("({converted_expr}).last")
            }
        }
    }
}

const COMMAND_IMPORT_HELPER_PKL: &str = r##"
local function __resolveCmdUri(rawUri: String): String =
  if (rawUri.matches(Regex(#"^[A-Za-z][A-Za-z0-9+.-]*:"#))) rawUri
  else if (rawUri.startsWith("/")) "file://" + rawUri
  else (if (__workingDirUri.endsWith("/")) __workingDirUri else __workingDirUri + "/") + rawUri

local function __resolveOneImport(imp: __pklCmdMod.Import): Any =
  let (resolved = __resolveCmdUri(imp.uri))
  if (imp.glob) import*(resolved) else import(resolved)

local function __resolveCmdImports(v: Any): Any =
  if (v is __pklCmdMod.Import) __resolveOneImport(v)
  else if (v is Pair) Pair(
    if (v.first is __pklCmdMod.Import) __resolveOneImport(v.first) else v.first,
    if (v.second is __pklCmdMod.Import) __resolveOneImport(v.second) else v.second
  )
  else if (v is List) v.map((e) -> if (e is __pklCmdMod.Import) __resolveOneImport(e) else e)
  else if (v is Set) v.map((e) -> if (e is __pklCmdMod.Import) __resolveOneImport(e) else e)
  else if (v is Map) v.toList().toMap(
    (e) -> if (e.first is __pklCmdMod.Import) __resolveOneImport(e.first) else e.first,
    (e) -> if (e.second is __pklCmdMod.Import) __resolveOneImport(e.second) else e.second
  )
  else v
"##;

fn pkl_quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn pkl_ident(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "abstract",
        "amends",
        "as",
        "class",
        "const",
        "else",
        "extends",
        "external",
        "false",
        "fixed",
        "for",
        "function",
        "hidden",
        "if",
        "import",
        "in",
        "is",
        "let",
        "local",
        "module",
        "new",
        "nothing",
        "null",
        "open",
        "out",
        "outer",
        "read",
        "super",
        "this",
        "throw",
        "trace",
        "true",
        "typealias",
        "unknown",
        "when",
    ];
    let valid_bare = !name.is_empty()
        && !KEYWORDS.contains(&name)
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if valid_bare {
        name.to_string()
    } else {
        format!("`{name}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtin_command_values() {
        assert_eq!(
            parse_single_value("yes", &CommandValueKind::Boolean).unwrap(),
            ParsedOptionValue::Bool(true)
        );
        assert_eq!(
            parse_single_value("OFF", &CommandValueKind::Boolean).unwrap(),
            ParsedOptionValue::Bool(false)
        );
        assert!(parse_single_value("maybe", &CommandValueKind::Boolean).is_err());
        assert_eq!(
            parse_single_value("42", &CommandValueKind::Number).unwrap(),
            ParsedOptionValue::Int(42)
        );
        assert_eq!(
            parse_single_value("3.5", &CommandValueKind::Number).unwrap(),
            ParsedOptionValue::Float(3.5)
        );
        assert!(
            parse_single_value(
                "300",
                &CommandValueKind::Int {
                    min: 0,
                    max: 255,
                    metavar: "uint8",
                }
            )
            .is_err()
        );
        assert_eq!(
            parse_single_value(
                "k=10",
                &CommandValueKind::Pair {
                    first: Box::new(CommandValueKind::String),
                    second: Box::new(CommandValueKind::Int {
                        min: i64::MIN,
                        max: i64::MAX,
                        metavar: "int",
                    }),
                }
            )
            .unwrap(),
            ParsedOptionValue::Pair(
                Box::new(ParsedOptionValue::String("k".into())),
                Box::new(ParsedOptionValue::Int(10))
            )
        );
    }
}
