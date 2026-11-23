//! Evaluator-owned standard-library modules share ordinary module/class/member
//! identities and the same code arena as user declarations. External bindings
//! retain their pinned declaration source; missing bindings fail explicitly.
use super::*;
use crate::modules::{ModuleIdentity, UnitId};
use crate::syntax::ExprKind;
use crate::value::RuntimeSource;

impl Vm {
    pub(super) fn stdlib_source(
        &self,
        uri: impl Into<PklString>,
    ) -> Result<Arc<RuntimeSource>, RuntimeError> {
        let uri = uri.into();
        let uri = &uri;
        Ok(match uri.as_str() {
            Some("pkl:base") => crate::stdlib_sources::base(),
            Some("pkl:test") => crate::stdlib_sources::test(),
            Some("pkl:ref") => crate::stdlib_sources::reference(),
            Some("pkl:analyze") => crate::stdlib_sources::analyze(),
            Some("pkl:reflect") => crate::stdlib_sources::reflect(),
            Some("pkl:semver") => crate::stdlib_sources::semver(),
            Some("pkl:EvaluatorSettings") => crate::stdlib_sources::evaluator_settings(),
            Some("pkl:Project" | "pkl:project") => crate::stdlib_sources::project(),
            Some("pkl:platform") => crate::stdlib_sources::platform(),
            Some("pkl:release") => crate::stdlib_sources::release(),
            Some("pkl:shell") => crate::stdlib_sources::shell(),
            Some("pkl:protobuf") => crate::stdlib_sources::protobuf(),
            Some("pkl:xml") => crate::stdlib_sources::xml(),
            Some("pkl:jsonnet") => crate::stdlib_sources::jsonnet(),
            Some("pkl:pklbinary") => crate::stdlib_sources::pklbinary(),
            Some("pkl:json") => crate::stdlib_sources::json(),
            Some("pkl:yaml") => crate::stdlib_sources::yaml(),
            Some("pkl:math") => crate::stdlib_sources::math(),
            Some("pkl:Benchmark") => crate::stdlib_sources::benchmark(),
            Some("pkl:Command") => crate::stdlib_sources::command(),
            Some("pkl:DocPackageInfo") => crate::stdlib_sources::doc_package_info(),
            Some("pkl:DocsiteInfo") => crate::stdlib_sources::docsite_info(),
            Some("pkl:settings") => crate::stdlib_sources::settings(),
            _ if uri.starts_with("pkl:") => {
                if crate::modules::stdlib_kind(uri) != Some(crate::modules::StdlibKind::Missing) {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        PklString::from("Standard-library module `")
                            .concat(uri)
                            .concat(&"` is not implemented.".into()),
                    ));
                }
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Cannot find standard library module `pkl:")
                        .concat(
                            &crate::host::resources::decoded_scheme_specific_part(uri)
                                .unwrap_or_else(|| {
                                    crate::source::SourceText::from(uri)
                                        .entire()
                                        .strip_prefix("pkl:")
                                        .expect("pkl URI prefix")
                                        .to_pkl_string()
                                }),
                        )
                        .concat(&"`.".into()),
                );
                error.hint = Some(
                    format!(
                        "Available standard library modules:\n{}",
                        include_str!("../stdlib/modules.txt")
                            .strip_suffix('\n')
                            .unwrap()
                    )
                    .into(),
                );
                return Err(error);
            }
            _ => return Err(invalid("not a bundled standard-library URI")),
        })
    }

    pub(super) fn stdlib_entry(
        &mut self,
        uri: impl Into<PklString>,
    ) -> Result<usize, RuntimeError> {
        let uri = uri.into();
        let uri = &uri;
        if uri.as_str() == Some("pkl:reflect") {
            self.ensure_base_full_metadata()?;
        }
        let source = self.stdlib_source(uri)?;
        let power_assertions = self.power_assertions;
        let program = Arc::make_mut(&mut self.program);
        let unit = append_definition(
            program,
            uri.as_str().expect("selected pinned builtin URI"),
            source,
            power_assertions,
        )?;
        Ok(program.units[unit.0 as usize].entry)
    }

    // Implicit prelude bindings are not authored ImportNodes and have no import
    // edge. Explicit imports go through ModuleRun before reaching this source.
    pub(super) fn import_stdlib(
        &mut self,
        uri: impl Into<PklString>,
    ) -> Result<bool, RuntimeError> {
        let uri = uri.into();
        let uri = &uri;
        if uri.as_str() == Some("pkl:reflect") {
            self.ensure_base_full_metadata()?;
        }
        let cache = if matches!(
            crate::modules::stdlib_kind(uri),
            Some(crate::modules::StdlibKind::Singleton(_))
        ) {
            &self.modules_stdlib
        } else {
            &self.modules_original
        };
        if let Some(cached) = cache.get(uri).cloned() {
            let value = self.module_cached(cached)?;
            self.stack.push(value);
            return Ok(true);
        }
        let entry = self.stdlib_entry(uri)?;
        // The ordinary module initializer publishes its receiver before forcing
        // pinned metadata and supported declarations, just like loaded modules.
        self.push_module_initializer(entry, None);
        Ok(true)
    }

    pub(super) fn in_stdlib_bootstrap(&self) -> bool {
        self.frames.iter().any(|frame| {
            self.program
                .metadata
                .get(frame.block)
                .is_some_and(|metadata| metadata.stdlib_bootstrap)
        })
    }

    pub(super) fn bootstrap_stdlib(&mut self) -> Result<(), RuntimeError> {
        let root = self.pop()?;
        self.force_stdlib_frame(root)
    }

    /// Lazily populates full `pkl:base` AST syntax (`ModuleUnit.syntax`) and
    /// complete declaration sources (including doc comments and class members)
    /// when reflection (`pkl:reflect`) or schema extraction is requested.
    /// Compilation of `pkl:base` reflection annotation collectors is deferred
    /// further to `ensure_base_reflection_annotations` so reflection workloads
    /// that never read `.annotations` on `pkl:base` declarations do not pay for
    /// resolving all 126 KB of `base.pkl`.
    pub(crate) fn ensure_base_full_metadata(&mut self) -> Result<(), RuntimeError> {
        if self.program.base_full_metadata_loaded {
            return Ok(());
        }
        let base_unit = self.program.units.iter().find_map(|unit| {
            (unit.standard_library && unit.singleton_cache_key == Some("pkl:base"))
                .then_some(unit.id)
        });
        let program = Arc::make_mut(&mut self.program);
        program.require_base_full_metadata = true;
        let Some(unit) = base_unit else {
            return Ok(());
        };
        if program.base_full_metadata_loaded {
            return Ok(());
        }
        let source = crate::stdlib_sources::base();
        let parsed = cached_parsed_stdlib("pkl:base", &source)?;
        program.seal_code_segment();
        program.units[unit.0 as usize].syntax = Some(Arc::clone(&parsed));
        let template_members = program.objects[program.units[unit.0 as usize].template].clone();
        let mut upgraded_decls = BTreeMap::new();
        for declaration in &parsed.members {
            let method = matches!(
                declaration.expr.kind,
                ExprKind::Function { method: true, .. }
            );
            let is_import = matches!(declaration.expr.kind, ExprKind::Import { .. });
            if let Some(member) = template_members.iter().find(|member| {
                member.name == declaration.name
                    && member.method == method
                    && member.import() == is_import
            }) {
                upgraded_decls.insert(member.code, declaration.declaration_source());
            }
        }
        if let Some(base_decls) = &mut program.base_declaration_sources {
            Arc::make_mut(base_decls).extend(upgraded_decls);
        } else {
            program.declaration_sources.extend(upgraded_decls);
        }
        program.seal_code_segment();
        program.base_full_metadata_loaded = true;
        Ok(())
    }

    /// Lazily resolves `pkl:base` and compiles its reflection annotation
    /// collectors on first read of `.annotations` on a `pkl:base` declaration
    /// or schema extraction of `pkl:base`.
    pub(crate) fn ensure_base_reflection_annotations(&mut self) -> Result<(), RuntimeError> {
        if self.program.base_reflection_annotations_loaded {
            return Ok(());
        }
        self.ensure_base_full_metadata()?;
        let base_unit = self.program.units.iter().find_map(|unit| {
            (unit.standard_library && unit.singleton_cache_key == Some("pkl:base"))
                .then_some(unit.id)
        });
        let Some(unit) = base_unit else {
            return Ok(());
        };
        let program = Arc::make_mut(&mut self.program);
        if program.base_reflection_annotations_loaded {
            return Ok(());
        }
        let source = crate::stdlib_sources::base();
        let parsed = cached_parsed_stdlib("pkl:base", &source)?;
        program.seal_code_segment();
        let prev_refl = std::mem::take(&mut program.reflection_annotations);
        compile_reflection_annotations(program, unit, "pkl:base", source, &parsed, &[])?;
        let base_refl = std::mem::replace(&mut program.reflection_annotations, prev_refl);
        if let Some(base_slot) = &mut program.base_reflection_annotations {
            Arc::make_mut(base_slot).extend(base_refl);
        } else {
            program.reflection_annotations.extend(base_refl);
        }
        program.seal_code_segment();
        program.base_reflection_annotations_loaded = true;
        Ok(())
    }
}

fn cached_base_top_level(source: &crate::value::RuntimeSource) -> Arc<crate::syntax::Module> {
    static CELL: std::sync::OnceLock<Arc<crate::syntax::Module>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        Arc::new(
            crate::parser::parse_base_top_level(&source.text)
                .expect("bundled pkl:base top-level parses cleanly"),
        )
    })
    .clone()
}

fn cached_base_top_level_resolved(
    parsed: &crate::syntax::Module,
) -> Arc<crate::resolver::ResolvedModule> {
    static CELL: std::sync::OnceLock<Arc<crate::resolver::ResolvedModule>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        Arc::new(
            crate::resolver::resolve_stdlib_module(parsed)
                .expect("bundled pkl:base top-level resolves cleanly"),
        )
    })
    .clone()
}

fn cached_parsed_stdlib(
    uri: &str,
    source: &crate::value::RuntimeSource,
) -> Result<Arc<crate::syntax::Module>, RuntimeError> {
    use std::sync::OnceLock;
    macro_rules! cached_cell {
        ($src:expr) => {{
            static CELL: OnceLock<Arc<crate::syntax::Module>> = OnceLock::new();
            Ok(CELL
                .get_or_init(|| {
                    Arc::new(
                        crate::parser::parse_module(&$src.text)
                            .expect("bundled standard-library module parses cleanly"),
                    )
                })
                .clone())
        }};
    }
    match uri {
        "pkl:base" => cached_cell!(source),
        "pkl:test" => cached_cell!(source),
        "pkl:ref" => cached_cell!(source),
        "pkl:analyze" => cached_cell!(source),
        "pkl:reflect" => cached_cell!(source),
        "pkl:semver" => cached_cell!(source),
        "pkl:EvaluatorSettings" => cached_cell!(source),
        "pkl:Project" | "pkl:project" => cached_cell!(source),
        "pkl:platform" => cached_cell!(source),
        "pkl:release" => cached_cell!(source),
        "pkl:shell" => cached_cell!(source),
        "pkl:protobuf" => cached_cell!(source),
        "pkl:xml" => cached_cell!(source),
        "pkl:jsonnet" => cached_cell!(source),
        "pkl:pklbinary" => cached_cell!(source),
        "pkl:json" => cached_cell!(source),
        "pkl:yaml" => cached_cell!(source),
        "pkl:math" => cached_cell!(source),
        "pkl:Benchmark" => cached_cell!(source),
        "pkl:Command" => cached_cell!(source),
        "pkl:DocPackageInfo" => cached_cell!(source),
        "pkl:DocsiteInfo" => cached_cell!(source),
        "pkl:settings" => cached_cell!(source),
        _ => crate::parser::parse_module(&source.text)
            .map(Arc::new)
            .map_err(|error| {
                RuntimeError::new(
                    ErrorKind::Unsupported,
                    PklString::from("Standard-library declaration parsing: ")
                        .concat(&error.message),
                )
            }),
    }
}

fn cached_resolved_stdlib(
    uri: &str,
    parsed: &crate::syntax::Module,
) -> Result<Arc<crate::resolver::ResolvedModule>, RuntimeError> {
    use std::sync::OnceLock;
    macro_rules! cached_cell {
        ($parsed:expr) => {{
            static CELL: OnceLock<Arc<crate::resolver::ResolvedModule>> = OnceLock::new();
            Ok(CELL
                .get_or_init(|| {
                    Arc::new(
                        crate::resolver::resolve_stdlib_module($parsed)
                            .expect("bundled standard-library module resolves cleanly"),
                    )
                })
                .clone())
        }};
    }
    match uri {
        "pkl:base" => cached_cell!(parsed),
        "pkl:test" => cached_cell!(parsed),
        "pkl:ref" => cached_cell!(parsed),
        "pkl:analyze" => cached_cell!(parsed),
        "pkl:reflect" => cached_cell!(parsed),
        "pkl:semver" => cached_cell!(parsed),
        "pkl:EvaluatorSettings" => cached_cell!(parsed),
        "pkl:Project" | "pkl:project" => cached_cell!(parsed),
        "pkl:platform" => cached_cell!(parsed),
        "pkl:release" => cached_cell!(parsed),
        "pkl:shell" => cached_cell!(parsed),
        "pkl:protobuf" => cached_cell!(parsed),
        "pkl:xml" => cached_cell!(parsed),
        "pkl:jsonnet" => cached_cell!(parsed),
        "pkl:pklbinary" => cached_cell!(parsed),
        "pkl:json" => cached_cell!(parsed),
        "pkl:yaml" => cached_cell!(parsed),
        "pkl:math" => cached_cell!(parsed),
        "pkl:Benchmark" => cached_cell!(parsed),
        "pkl:Command" => cached_cell!(parsed),
        "pkl:DocPackageInfo" => cached_cell!(parsed),
        "pkl:DocsiteInfo" => cached_cell!(parsed),
        "pkl:settings" => cached_cell!(parsed),
        _ => crate::resolver::resolve_stdlib_module(parsed)
            .map(Arc::new)
            .map_err(|error| RuntimeError::new(ErrorKind::Unsupported, error.message)),
    }
}

fn cached_base_header_resolved(
    parsed: &crate::syntax::Module,
) -> Arc<crate::resolver::ResolvedModule> {
    static CELL: std::sync::OnceLock<Arc<crate::resolver::ResolvedModule>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut header = parsed.clone();
        header.members.retain(|member| {
            matches!(member.expr.kind, ExprKind::Import { .. })
                || matches!(
                    member
                        .name
                        .as_str()
                        .expect("pinned bundled declaration name"),
                    "DataSizeUnit"
                        | "DurationUnit"
                        | "Charset"
                        | "Comparable"
                        | "Char"
                        | "Uri"
                        | "Mixin"
                        | "TODO"
                        | "Undefined"
                )
        });
        Arc::new(
            crate::resolver::resolve_stdlib_module(&header)
                .expect("bundled pkl:base header resolves cleanly"),
        )
    })
    .clone()
}

fn append_definition(
    program: &mut Program,
    uri: &str,
    source: Arc<crate::value::RuntimeSource>,
    power_assertions: bool,
) -> Result<UnitId, RuntimeError> {
    let singleton_cache_key = match crate::modules::stdlib_kind(&uri.into()) {
        Some(crate::modules::StdlibKind::Singleton(canonical)) => Some(canonical),
        _ => None,
    };
    if let Some(unit) = program.units.iter().find(|unit| {
        unit.standard_library
            && unit.singleton_cache_key == singleton_cache_key
            && (singleton_cache_key.is_some()
                || unit
                    .identity
                    .as_ref()
                    .is_some_and(|identity| identity.original_uri == uri))
    }) {
        return Ok(unit.id);
    }
    let fast_base = uri == "pkl:base" && !program.require_base_full_metadata;
    let parsed = if fast_base {
        cached_base_top_level(&source)
    } else {
        cached_parsed_stdlib(uri, &source)?
    };
    let checkpoint = modules::ArenaCheckpoint::new(program);
    let result = append_members(program, uri, source, &parsed, power_assertions).inspect(|unit| {
        let entry = &mut program.units[unit.0 as usize];
        entry.standard_library = true;
        entry.singleton_cache_key = singleton_cache_key;
    });
    if result.is_err() {
        checkpoint.rollback(program);
    }
    result
}

fn append_members(
    program: &mut Program,
    uri: &str,
    source: Arc<crate::value::RuntimeSource>,
    parsed: &Arc<crate::syntax::Module>,
    power_assertions: bool,
) -> Result<UnitId, RuntimeError> {
    let lazy_class_bootstrap = matches!(
        uri,
        "pkl:EvaluatorSettings"
            | "pkl:Project"
            | "pkl:project"
            | "pkl:DocPackageInfo"
            | "pkl:DocsiteInfo"
            | "pkl:Benchmark"
            | "pkl:reflect"
    );
    if lazy_class_bootstrap
        || matches!(
            uri,
            "pkl:analyze"
                | "pkl:platform"
                | "pkl:release"
                | "pkl:semver"
                | "pkl:json"
                | "pkl:jsonnet"
                | "pkl:protobuf"
                | "pkl:xml"
                | "pkl:yaml"
                | "pkl:shell"
                | "pkl:pklbinary"
                | "pkl:Command"
                | "pkl:settings"
        )
    {
        // Compile the actual module, including distinct class/property and
        // method namespaces. External bodies bind in the shared compiler.
        // Publish the singleton before recursively forcing its constants, as
        // StdLibModule.doLoad does for the JVM's statically shared modules.
        let resolved = cached_resolved_stdlib(uri, parsed)?;
        let identity_uri = source.uri.clone();
        let unit = compile_unit_into(
            program,
            &resolved,
            CompileOptions {
                fuse_constant_add: true,
            },
            power_assertions,
            Some(source),
            Some(ModuleIdentity {
                original_uri: identity_uri.clone(),
                resolved_uri: identity_uri,
            }),
        )?;
        // These modules declare classes eagerly while keeping ordinary module
        // properties lazy: Project.resolvedEvaluatorSettings and Benchmark.report
        // must not require platform.current merely because the module is imported,
        // and DocPackageInfo/DocsiteInfo retain undefined template properties.
        if !lazy_class_bootstrap {
            append_bootstrap(program, unit, uri);
        } else {
            append_class_bootstrap(program, unit, parsed);
            if uri == "pkl:reflect" {
                append_reflect_const_bootstrap(program, unit, parsed);
            }
        }
        return Ok(unit);
    }
    // Compile the real authored annotation/initialization path. External member
    // bodies are bound below; they are not rewritten into Pkl source strings.
    let resolved = if uri == "pkl:base" {
        cached_base_header_resolved(parsed)
    } else {
        let mut header = (**parsed).clone();
        header.members.clear();
        Arc::new(
            crate::resolver::resolve_stdlib_module(&header)
                .map_err(|error| RuntimeError::new(ErrorKind::Unsupported, error.message))?,
        )
    };
    let unit = compile_unit_into(
        program,
        &resolved,
        CompileOptions {
            fuse_constant_add: true,
        },
        false,
        Some(source.clone()),
        Some(ModuleIdentity {
            original_uri: uri.into(),
            resolved_uri: uri.into(),
        }),
    )?;
    // Methods are published during module initialization, so they are absent
    // from initial_members. Preserve their compiled bodies from the template.
    let compiled_members = program.objects[program.units[unit.0 as usize].template].clone();
    let mut members = Vec::new();
    let mut synthetic_annotated = Vec::new();
    for (index, declaration) in parsed.members.iter().enumerate() {
        let method = matches!(
            declaration.expr.kind,
            ExprKind::Function { method: true, .. }
        );
        let is_import = matches!(declaration.expr.kind, ExprKind::Import { .. });
        if let Some(member) = compiled_members.iter().find(|member| {
            member.name == declaration.name
                && member.method == method
                && member.import() == is_import
        }) {
            members.push(member.clone());
            continue;
        }
        let mut ty = None;
        let mut type_span = None;
        let mut body = if uri == "pkl:base" {
            base_binding(program, unit, source.clone(), declaration)?
        } else if uri == "pkl:math" {
            match primitives::math_module::binding(program, unit, source.clone(), declaration)? {
                Some(body) => body,
                None => math_binding(declaration),
            }
        } else if uri == "pkl:ref" {
            match declaration
                .name
                .as_str()
                .expect("pinned bundled declaration name")
            {
                "Reference" if method => vec![
                    Instruction::LoadRefModule,
                    Instruction::GetMethod("Reference".into()),
                ],
                "Reference" if !method => {
                    vec![Instruction::BuiltinClass(Box::new(TypeCheck::Reference {
                        domain: Box::new(TypeCheck::Any),
                        referent: Box::new(TypeCheck::Any),
                    }))]
                }
                "Domain" if !method => {
                    vec![Instruction::BuiltinClass(Box::new(TypeCheck::RefDomain))]
                }
                "Access" if !method => {
                    vec![Instruction::BuiltinClass(Box::new(TypeCheck::Class {
                        module: None,
                        id: reference::ACCESS_ID,
                        name: "Access".into(),
                    }))]
                }
                _ => unsupported_binding(uri, declaration),
            }
        } else {
            match declaration
                .name
                .as_str()
                .expect("pinned bundled declaration name")
            {
                "facts" | "examples" if !method => {
                    let value_type = if declaration.name == "facts" {
                        TypeCheck::Boolean
                    } else {
                        TypeCheck::Unknown
                    };
                    let listing = TypeCheck::ParameterizedCollection {
                        kind: crate::heap::CollectionKind::Listing,
                        arguments: vec![(value_type, declaration.ty.as_ref().unwrap().span)],
                    };
                    let mapping = TypeCheck::ParameterizedCollection {
                        kind: crate::heap::CollectionKind::Mapping,
                        arguments: vec![
                            (TypeCheck::String, declaration.ty.as_ref().unwrap().span),
                            (listing, declaration.ty.as_ref().unwrap().span),
                        ],
                    };
                    let check = TypeCheck::Nullable(Box::new(mapping.clone()));
                    ty = Some(check.clone());
                    type_span = declaration.ty.as_ref().map(|ty| ty.span);
                    // A nullable Mapping retains its collection default for amendments.
                    vec![Instruction::ResolvedDefault {
                        ty: Box::new(check),
                        origin: Some(Box::new(RuntimeFrame {
                            anonymous: false,
                            source: Some(source.clone()),
                            span: type_span.unwrap(),
                            member: format!("#{}", declaration.name).into(),
                        })),
                    }]
                }
                "catch" | "catchOrNull" if method => {
                    let ExprKind::Function {
                        parameters,
                        header_span,
                        ..
                    } = &declaration.expr.kind
                    else {
                        unreachable!()
                    };
                    let argument_span = parameters[0]
                        .ty
                        .as_ref()
                        .expect("pinned catch argument")
                        .span;
                    let block = program.alloc_block(
                        vec![
                            Instruction::LoadLocal(0),
                            Instruction::CheckType(Box::new(TypeCheck::Function {
                                parameters: Some(Vec::new()),
                                result: Box::new(TypeCheck::Any),
                                syntax: true,
                            })),
                            Instruction::GuestCatch {
                                or_null: declaration.name == "catchOrNull",
                            },
                            Instruction::Return,
                        ],
                        BlockMetadata {
                            unit: Some(unit),
                            source: Some(source.clone()),
                            member: format!("#{}", declaration.name).into(),
                            declaration: Some(*header_span),
                            spans: vec![None, Some(argument_span), None, None],
                            ..Default::default()
                        },
                    )?;
                    vec![Instruction::MakeFunction {
                        block,
                        arity: 1,
                        method: true,
                        capture_custom_this: false,
                    }]
                }
                _ => unsupported_binding(uri, declaration),
            }
        };
        body.push(Instruction::Return);
        let spans = vec![Some(declaration.name_span); body.len()];
        let method_signature = if let ExprKind::Function {
            parameters,
            method: true,
            ..
        } = &declaration.expr.kind
        {
            Some((
                declaration
                    .name
                    .concat(&"(".into())
                    .concat(&PklString::join(
                        &parameters
                            .iter()
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>(),
                        ", ",
                    ))
                    .concat(&")".into()),
                parameters.len(),
            ))
        } else {
            None
        };
        let block = program.alloc_block(
            body,
            BlockMetadata {
                unit: Some(unit),
                source: Some(source.clone()),
                member: format!("#{}", declaration.name).into(),
                declaration: Some(declaration.name_span),
                method_signature,
                spans,
                ..Default::default()
            },
        )?;
        program
            .declaration_sources
            .insert(block, declaration.declaration_source());
        if !declaration.annotations.is_empty()
            && !matches!(
                declaration.expr.kind,
                ExprKind::Class { .. } | ExprKind::TypeAlias { .. }
            )
        {
            synthetic_annotated.push((index, block, method));
        }
        members.push(Member {
            flags: Member::declaration_flags(declaration),
            entry: false,
            name: declaration.name.clone(),
            code: block,
            local: declaration.local,
            method,
            ty,
            type_span,
            constant: declaration
                .modifiers
                .iter()
                .any(|modifier| modifier.kind == crate::syntax::ModifierKind::Const)
                || matches!(
                    declaration.expr.kind,
                    ExprKind::Class { .. } | ExprKind::TypeAlias { .. }
                ),
            fixed: false,
        });
    }
    let fast_base = uri == "pkl:base" && !program.require_base_full_metadata;
    let definition = &mut program.units[unit.0 as usize];
    program.objects[definition.template] = members.clone();
    definition.initial_members = members;
    if !fast_base {
        definition.syntax = Some(Arc::clone(parsed));
    }
    if fast_base {
        compile_base_synthetic_annotations_only(
            program,
            unit,
            source,
            parsed,
            &synthetic_annotated,
        )?;
    } else {
        compile_reflection_annotations(program, unit, uri, source, parsed, &synthetic_annotated)?;
        if uri == "pkl:base" {
            program.base_full_metadata_loaded = true;
            program.base_reflection_annotations_loaded = true;
        }
    }
    append_bootstrap(program, unit, uri);
    Ok(unit)
}

fn append_bootstrap(program: &mut Program, unit: UnitId, uri: &str) {
    let entry = program.units[unit.0 as usize].entry;
    let finish = program.blocks[entry]
        .iter()
        .position(|instruction| matches!(instruction, Instruction::FinishModule))
        .expect("ordinary module initializer");
    let mut bootstrap = vec![
        Instruction::Duplicate,
        Instruction::BootstrapStdlib,
        Instruction::Pop,
    ];
    if uri == "pkl:base" {
        bootstrap.extend([
            Instruction::Duplicate,
            Instruction::GetMember("output".into()),
            Instruction::BootstrapStdlib,
            Instruction::Pop,
        ]);
    }
    program.metadata[entry].stdlib_bootstrap = true;
    program.metadata[entry]
        .spans
        .splice(finish..finish, vec![None; bootstrap.len()]);
    program.blocks[entry].splice(finish..finish, bootstrap);
}

fn append_class_bootstrap(program: &mut Program, unit: UnitId, parsed: &crate::syntax::Module) {
    let entry = program.units[unit.0 as usize].entry;
    let finish = program.blocks[entry]
        .iter()
        .position(|instruction| matches!(instruction, Instruction::FinishModule))
        .expect("ordinary module initializer");
    let mut bootstrap = Vec::new();
    for member in &parsed.members {
        if matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. }) {
            bootstrap.push(Instruction::Duplicate);
            bootstrap.push(Instruction::GetMember(member.name.clone()));
            bootstrap.push(Instruction::Pop);
        }
    }
    if !bootstrap.is_empty() {
        program.metadata[entry]
            .spans
            .splice(finish..finish, vec![None; bootstrap.len()]);
        program.blocks[entry].splice(finish..finish, bootstrap);
    }
}

/// For `pkl:reflect`, defer forcing each `const ...Type` standard mirror until
/// that specific property is read instead of eagerly forcing all 27 mirrors on
/// `import "pkl:reflect"`. When a property is read, run `BootstrapStdlib` on
/// its own initializer frame with `stdlib_bootstrap = true` so recursive
/// forcing and `FrameTransformer::Default` isolation match Apple Pkl 0.32.0.
fn append_reflect_const_bootstrap(
    program: &mut Program,
    unit: UnitId,
    parsed: &crate::syntax::Module,
) {
    let template = program.units[unit.0 as usize].template;
    let const_codes: Vec<usize> = parsed
        .members
        .iter()
        .filter(|decl| {
            !matches!(
                decl.expr.kind,
                ExprKind::Class { .. }
                    | ExprKind::TypeAlias { .. }
                    | ExprKind::Function { method: true, .. }
                    | ExprKind::Import { .. }
            )
        })
        .filter_map(|decl| {
            program.objects[template]
                .iter()
                .find(|member| !member.method && !member.import() && member.name == decl.name)
                .map(|member| member.code)
        })
        .collect();
    for block in const_codes {
        let ret = program.blocks[block]
            .len()
            .checked_sub(1)
            .expect("non-empty const property block");
        assert!(
            matches!(program.blocks[block][ret], Instruction::Return),
            "reflect const property block must end with Return"
        );
        let bootstrap = [
            Instruction::Duplicate,
            Instruction::BootstrapStdlib,
            Instruction::Pop,
        ];
        program.metadata[block].stdlib_bootstrap = true;
        program.metadata[block]
            .spans
            .splice(ret..ret, [None, None, None]);
        program.blocks[block].splice(ret..ret, bootstrap);
    }
}

fn base_binding(
    program: &mut Program,
    unit: UnitId,
    source: Arc<RuntimeSource>,
    declaration: &crate::syntax::Member,
) -> Result<Vec<Instruction>, RuntimeError> {
    let name = declaration
        .name
        .as_str()
        .expect("pinned bundled declaration name");
    match &declaration.expr.kind {
        ExprKind::Function { method: true, .. } if name == "Regex" => {
            return Ok(vec![Instruction::RegexConstructor]);
        }
        ExprKind::Function { method: true, .. } if name == "Pair" => {
            return Ok(vec![Instruction::PairConstructor]);
        }
        ExprKind::Function { method: true, .. } if name == "IntSeq" => {
            return Ok(vec![Instruction::IntSeqConstructor]);
        }
        ExprKind::Function { method: true, .. } if name == "Null" => {
            return Ok(vec![Instruction::NullConstructor]);
        }
        // Qualified calls (e.g. `import("pkl:base").List(...)`) do not expand
        // VarArgs in Pkl 0.32.0: they invoke a 1-arg method whose parameter
        // type check rejects `VarArgs` outside stdlib expansion (BaseNodes.java).
        ExprKind::Function {
            method: true,
            parameters,
            header_span,
            ..
        } if matches!(name, "List" | "Set" | "Map" | "Bytes") => {
            let argument_span = parameters[0]
                .ty
                .as_ref()
                .expect("pinned VarArgs constructor parameter")
                .span;
            let block = program.alloc_block(
                vec![
                    Instruction::Fail(Box::new(RuntimeError::new(
                        ErrorKind::Type,
                        "Class `VarArgs` can only be used by the standard library.",
                    ))),
                    Instruction::Return,
                ],
                BlockMetadata {
                    unit: Some(unit),
                    source: Some(source),
                    member: format!("#{name}").into(),
                    declaration: Some(*header_span),
                    spans: vec![Some(argument_span), None],
                    ..Default::default()
                },
            )?;
            return Ok(vec![Instruction::MakeFunction {
                block,
                arity: 1,
                method: true,
                capture_custom_this: false,
            }]);
        }
        ExprKind::Class { .. } => {
            if let Some(ty) =
                reference::base_type(name).or_else(|| builtins::id(name).map(builtins::ty))
            {
                return Ok(vec![Instruction::BuiltinClass(Box::new(ty))]);
            }
        }
        ExprKind::TypeAlias { .. }
            if matches!(
                name,
                "NonNull" | "Int8" | "Int16" | "Int32" | "UInt8" | "UInt16" | "UInt32" | "UInt"
            ) =>
        {
            return Ok(vec![Instruction::BuiltinTypeAlias(name.into())]);
        }
        _ if matches!(name, "NaN" | "Infinity") => {
            return Ok(vec![Instruction::Constant(Value::Float(
                if name == "NaN" {
                    f64::NAN.into()
                } else {
                    f64::INFINITY.into()
                },
            ))]);
        }
        _ => {}
    }
    Ok(unsupported_binding("pkl:base", declaration))
}

// Every property constant declared by pinned MathNodes. External methods are
// bound separately through the shared math_module primitive.
fn math_binding(declaration: &crate::syntax::Member) -> Vec<Instruction> {
    let name = declaration
        .name
        .as_str()
        .expect("pinned bundled declaration name");
    let integer = match name {
        "minInt" => Some(i64::MIN),
        "maxInt" | "maxUInt" => Some(i64::MAX),
        "minInt8" => Some(i8::MIN as i64),
        "maxInt8" => Some(i8::MAX as i64),
        "minInt16" => Some(i16::MIN as i64),
        "maxInt16" => Some(i16::MAX as i64),
        "minInt32" => Some(i32::MIN as i64),
        "maxInt32" => Some(i32::MAX as i64),
        "maxUInt8" => Some(u8::MAX as i64),
        "maxUInt16" => Some(u16::MAX as i64),
        "maxUInt32" => Some(u32::MAX as i64),
        _ => None,
    };
    let value = if let Some(value) = integer {
        Value::Int(value.into())
    } else {
        Value::Float(
            match name {
                "minFiniteFloat" => -f64::MAX,
                "maxFiniteFloat" => f64::MAX,
                "minPositiveFloat" => f64::from_bits(1),
                "e" => std::f64::consts::E,
                "pi" => std::f64::consts::PI,
                _ => return unsupported_binding("pkl:math", declaration),
            }
            .into(),
        )
    };
    vec![Instruction::Constant(value)]
}

fn unsupported_binding(uri: &str, declaration: &crate::syntax::Member) -> Vec<Instruction> {
    vec![Instruction::Fail(Box::new(RuntimeError::new(
        ErrorKind::Unsupported,
        format!(
            "Standard-library binding `{uri}#{}` is not implemented.",
            declaration.name
        ),
    )))]
}

/// Fast cold-start path for `pkl:base`: compiles only the top-level
/// `synthetic_annotated` member annotations (e.g. `@Since` on `Bytes(...)`)
/// against the lightweight 5 KB top-level AST without resolving all 126 KB of
/// `base.pkl` or compiling the 98 reflection-only annotation collectors.
fn compile_base_synthetic_annotations_only(
    program: &mut Program,
    unit: UnitId,
    source: Arc<RuntimeSource>,
    parsed: &crate::syntax::Module,
    synthetic_annotated: &[(usize, usize, bool)],
) -> Result<(), RuntimeError> {
    if synthetic_annotated.is_empty() {
        return Ok(());
    }
    let resolved = cached_base_top_level_resolved(parsed);
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(source.text.len_bytes() + 2)
        .ok_or_else(|| invalid("annotation declaration identity limit"))?;
    let (declared_recipes, module_recipes) = Compiler::seed_recipe_caches(program, unit);
    let mut compiler = Compiler {
        resolved: &resolved,
        type_lookup: build_type_lookup(&resolved),
        program,
        unit,
        declaration_base,
        source: Some(source),
        identity: Some(ModuleIdentity {
            original_uri: "pkl:base".into(),
            resolved_uri: "pkl:base".into(),
        }),
        options: CompileOptions {
            fuse_constant_add: true,
        },
        member: PklString::default(),
        super_method_allowed: false,
        module_annotation: false,
        let_members: Vec::new(),
        type_scopes: vec![extract_type_scope(&resolved.module.members)],
        runtime_type_owner: true,
        runtime_type_fallback: None,
        modifiers: Vec::new(),
        constraint_module: false,
        alias_frames: Vec::new(),
        constraint_origins: BTreeMap::new(),
        active_constraints: BTreeMap::new(),
        shared_constraint_roots: BTreeMap::new(),
        active_type_parameters: BTreeMap::new(),
        constructor_parameters_bound: false,
        parameter_spans: BTreeMap::new(),
        parameter_label_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        declared_recipes,
        module_recipes,
        instrument: false,
    };
    let mut publish = Code::default();
    for &(index, code, method) in synthetic_annotated {
        let member = &resolved.module.members[index];
        compiler.member_annotations(member, &mut publish, false)?;
        if method {
            publish.push(Instruction::InitializeDeprecation);
            publish.push(Instruction::StoreDeprecation);
        }
        publish.push(Instruction::PublishMemberAnnotations { code });
    }
    let entry = compiler.program.units[unit.0 as usize].entry;
    let finish = compiler.program.blocks[entry]
        .iter()
        .position(|instruction| matches!(instruction, Instruction::FinishModule))
        .expect("ordinary module initializer");
    compiler.program.metadata[entry]
        .spans
        .splice(finish..finish, publish.spans);
    compiler.program.blocks[entry].splice(finish..finish, publish.instructions);
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);
    Ok(())
}

// Builtin declaration annotations have ordinary executable bodies even when
// their enclosing generic class uses native members. Compile only these exact
// annotation ASTs, retaining source spans and the standard module code owner.
fn compile_reflection_annotations(
    program: &mut Program,
    unit: UnitId,
    uri: &str,
    source: Arc<RuntimeSource>,
    parsed: &crate::syntax::Module,
    synthetic_annotated: &[(usize, usize, bool)],
) -> Result<(), RuntimeError> {
    let resolved = cached_resolved_stdlib(uri, parsed)?;
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(source.text.len_bytes() + 2)
        .ok_or_else(|| invalid("annotation declaration identity limit"))?;
    let (declared_recipes, module_recipes) = Compiler::seed_recipe_caches(program, unit);
    let mut compiler = Compiler {
        resolved: &resolved,
        type_lookup: build_type_lookup(&resolved),
        program,
        unit,
        declaration_base,
        source: Some(source),
        identity: Some(ModuleIdentity {
            original_uri: uri.into(),
            resolved_uri: uri.into(),
        }),
        options: CompileOptions {
            fuse_constant_add: true,
        },
        member: PklString::default(),
        super_method_allowed: false,
        module_annotation: false,
        let_members: Vec::new(),
        type_scopes: vec![extract_type_scope(&resolved.module.members)],
        runtime_type_owner: true,
        runtime_type_fallback: None,
        modifiers: Vec::new(),
        constraint_module: false,
        alias_frames: Vec::new(),
        constraint_origins: BTreeMap::new(),
        active_constraints: BTreeMap::new(),
        shared_constraint_roots: BTreeMap::new(),
        active_type_parameters: BTreeMap::new(),
        constructor_parameters_bound: false,
        parameter_spans: BTreeMap::new(),
        parameter_label_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        declared_recipes,
        module_recipes,
        instrument: false,
    };
    if !synthetic_annotated.is_empty() {
        let mut publish = Code::default();
        for &(index, code, method) in synthetic_annotated {
            let member = &resolved.module.members[index];
            compiler.member_annotations(member, &mut publish, false)?;
            if method {
                publish.push(Instruction::InitializeDeprecation);
                publish.push(Instruction::StoreDeprecation);
            }
            publish.push(Instruction::PublishMemberAnnotations { code });
        }
        let entry = compiler.program.units[unit.0 as usize].entry;
        let finish = compiler.program.blocks[entry]
            .iter()
            .position(|instruction| matches!(instruction, Instruction::FinishModule))
            .expect("ordinary module initializer");
        compiler.program.metadata[entry]
            .spans
            .splice(finish..finish, publish.spans);
        compiler.program.blocks[entry].splice(finish..finish, publish.instructions);
    }
    let mut pending: Vec<_> = parsed
        .members
        .iter()
        .rev()
        .map(|member| (PklString::from("#").concat(&member.name), member))
        .collect();
    while let Some((name, member)) = pending.pop() {
        if !member.annotations.is_empty() {
            compiler.member = name.clone();
            let mut code = Code::default();
            code.push(Instruction::BeginMemberAnnotations);
            compiler.annotations(&member.annotations, &mut code)?;
            code.push(Instruction::Return);
            let block = compiler.block(code, Some(member.declaration_source().header_span))?;
            compiler
                .program
                .reflection_annotations
                .insert((uri.into(), member.declaration_source().header_span), block);
        }
        if let ExprKind::Class { members, .. } = &member.expr.kind {
            pending.extend(
                members
                    .iter()
                    .rev()
                    .map(|member| (name.concat(&".".into()).concat(&member.name), member)),
            );
        }
    }
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
        ready_with_limit(vm, 100_000)
    }

    fn ready_reflection(vm: &mut Vm) -> Result<Value, RuntimeError> {
        // Eager standard mirrors exceed the smaller module bootstrap budget.
        // Keep the same single-instruction polling and GC stress condition.
        ready_with_limit(vm, 400_000)
    }

    fn ready_with_limit(vm: &mut Vm, limit: usize) -> Result<Value, RuntimeError> {
        for _ in 0..limit {
            match vm.poll(1) {
                VmPoll::Ready(value) => return value,
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::Module(request) => {
                    assert!(
                        matches!(
                            request.kind,
                            crate::modules::ModuleRequestKind::CheckImport { .. }
                                | crate::modules::ModuleRequestKind::SelectKey { .. }
                        ),
                        "singleton imports require admission and actual key selection"
                    );
                    vm.complete_module(
                        request.id,
                        crate::modules::default_completion(&request).unwrap(),
                    )
                    .unwrap();
                }
                event => panic!("stdlib initialization leaked Host work: {event:?}"),
            }
        }
        panic!("stdlib initialization did not settle")
    }

    #[test]
    fn reflection_bootstrap_forces_standard_mirrors_without_user_frame_callbacks() {
        let parsed = crate::parser::parse_module(
            r#"reflect = import("pkl:reflect")
class User {}
unknownSame = reflect.listType.typeArguments.first == reflect.unknownType
unknownSetSize = Set(reflect.listType.typeArguments.first, reflect.unknownType).length
regularSame = reflect.StringLiteralType("a") == reflect.StringLiteralType("a")
regularDifferent = reflect.StringLiteralType("a") == reflect.StringLiteralType("b")
userLocation = reflect.Class(User).location.displayUri
"#,
        )
        .unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(Arc::new(crate::value::RuntimeSource {
                    text: parsed.source.clone(),
                    uri: "file:///reflection-bootstrap.pkl".into(),
                    module_name: "main".into(),
                })),
                frame_transformer: crate::logging::FrameTransformer::Host,
                ..Default::default()
            },
        );
        vm.start(program.entry, vec![]).unwrap();
        let module = ready_reflection(&mut vm).unwrap().as_object().unwrap();
        vm.start_member(module, "reflect").unwrap();
        // `pkl:reflect` uses lazy class bootstrap so importing it does not eagerly
        // force all 24 `const ...Type` standard mirrors. Observable reads of those
        // standard mirrors must still resolve with the default transformer without
        // leaking user Host frame-transform callbacks.
        let reflect = ready_reflection(&mut vm).unwrap().as_object().unwrap();
        assert_eq!(
            vm.member_state(reflect, "intType").unwrap(),
            LazyState::Unevaluated,
            "pkl:reflect const standard mirrors must remain lazy on import"
        );
        let mut current = reflect;
        for name in ["intType", "referent", "location"] {
            vm.start_member(current, name).unwrap();
            current = ready_reflection(&mut vm).unwrap().as_object().unwrap();
        }
        vm.start_member(current, "displayUri").unwrap();
        let Value::String(uri) = ready_reflection(&mut vm).unwrap() else {
            panic!("standard declaration location displayUri was not a String")
        };
        assert!(
            uri.as_str()
                .unwrap()
                .starts_with("https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L")
        );
        // UnknownType has only the inherited external nullable property. Two
        // distinct instances compare/hash without requesting a nullable cache.
        for (name, expected) in [
            ("unknownSame", Value::Bool(true)),
            ("unknownSetSize", Value::Int(1.into())),
            ("regularSame", Value::Bool(true)),
            ("regularDifferent", Value::Bool(false)),
        ] {
            vm.start_member(module, name).unwrap();
            assert_eq!(ready_reflection(&mut vm).unwrap(), expected, "{name}");
        }
        // The isolated bootstrap policy must not leak into user declarations.
        vm.start_member(module, "userLocation").unwrap();
        let mut request = None;
        for _ in 0..100_000 {
            match vm.poll(1) {
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::TransformFrame(pending) => {
                    request = Some(pending);
                    break;
                }
                event => panic!("user location did not suspend for Host: {event:?}"),
            }
        }
        let request = request.expect("bounded user location Host callback");
        assert_eq!(request.frame.module_uri, "file:///reflection-bootstrap.pkl");
        vm.complete_frame_transform(request.id, Ok((*request.frame).clone()))
            .unwrap();
        assert_eq!(
            ready_reflection(&mut vm).unwrap(),
            Value::String("file:///reflection-bootstrap.pkl".into())
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

    #[test]
    fn pinned_annotations_and_base_output_share_heap_with_explicit_default_import_policy() {
        let parsed = crate::parser::parse_module(
            "base = import(\"pkl:base\")\ntesting = import(\"pkl:test\")\n",
        )
        .unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(Arc::new(crate::value::RuntimeSource {
                    text: parsed.source.clone(),
                    uri: "file:///stdlib-structure.pkl".into(),
                    module_name: "main".into(),
                })),
                ..Default::default()
            },
        );
        vm.start(program.entry, vec![]).unwrap();
        let module = ready(&mut vm).unwrap().as_object().unwrap();
        for name in ["base", "testing"] {
            vm.start_member(module, name).unwrap();
            let standard = ready(&mut vm).unwrap().as_object().unwrap();
            let annotations = vm.heap.get(standard).unwrap().annotations.clone();
            assert_eq!(annotations.len(), 1);
            let annotation = annotations[0].as_object().unwrap();
            let class = collections::typed_class(&vm.heap, annotation)
                .unwrap()
                .unwrap();
            assert_eq!(class.id, annotations::builtin_id("ModuleInfo").unwrap());
            assert_eq!(
                vm.member_state(annotation, "minPklVersion").unwrap(),
                LazyState::Completed(Value::String("0.32.0".into()))
            );
            if name == "base" {
                let LazyState::Completed(Value::Object(output)) =
                    vm.member_state(standard, "output").unwrap()
                else {
                    panic!("base output was not eager")
                };
                assert_eq!(
                    vm.member_state(output, "text").unwrap(),
                    LazyState::Completed(Value::String("".into()))
                );
                assert_eq!(
                    vm.member_state(output, "value").unwrap(),
                    LazyState::Completed(Value::Object(standard))
                );
                assert!(
                    vm.heap.get(standard).unwrap().forced,
                    "base initialization must complete recursive forcing"
                );
                vm.start_member(standard, "Char").unwrap();
                let Value::TypeAlias(char_alias) = ready(&mut vm).unwrap() else {
                    panic!("Char was not a type alias")
                };
                assert_eq!(
                    vm.heap.get(char_alias).unwrap().type_alias,
                    Some("pkl.base#Char".into())
                );
                assert!(vm.heap.get(char_alias).unwrap().type_alias_initialized);
                vm.collect(&[]);
                vm.start_member(standard, "Char").unwrap();
                assert_eq!(ready(&mut vm).unwrap(), Value::TypeAlias(char_alias));
            } else {
                assert!(matches!(
                    vm.member_state(standard, "facts").unwrap(),
                    LazyState::Completed(Value::NullWithDefault(_))
                ));
                assert_eq!(
                    vm.member_state(standard, "output").unwrap(),
                    LazyState::Unevaluated
                );
            }
            vm.collect(&[]);
            assert!(vm.heap.get(annotation).is_ok());
        }
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }

    #[test]
    fn parse_base_top_level_matches_full_parse_on_top_level_declarations() {
        let base = crate::stdlib_sources::base();
        let full = crate::parser::parse_module(&base.text).unwrap();
        let fast = crate::parser::parse_base_top_level(&base.text).unwrap();
        let fast_decl = fast.declaration.as_ref().expect("base module declaration");
        let mut full_decl = full.declaration.clone().expect("base module declaration");
        assert!(
            fast_decl.doc_comment.is_none(),
            "fast base top-level parse intentionally skips doc comments"
        );
        full_decl.doc_comment = None;
        assert_eq!(*fast_decl, full_decl);
        assert_eq!(fast.members.len(), full.members.len());
        for (f, g) in fast.members.iter().zip(&full.members) {
            assert_eq!(f.name, g.name);
            assert_eq!(f.name_span, g.name_span, "member {}", g.name);
            assert_eq!(f.span, g.span, "member {}", g.name);
            assert_eq!(f.local, g.local, "member {}", g.name);
            assert_eq!(f.modifiers, g.modifiers, "member {}", g.name);
            assert_eq!(
                f.annotations.len(),
                g.annotations.len(),
                "member {}",
                g.name
            );
            assert_eq!(
                f.declaration_source().header_span,
                g.declaration_source().header_span,
                "header_span for {}",
                g.name
            );
            match (&f.expr.kind, &g.expr.kind) {
                (
                    ExprKind::Function {
                        method: fm,
                        parameters: fp,
                        header_span: fh,
                        ..
                    },
                    ExprKind::Function {
                        method: gm,
                        parameters: gp,
                        header_span: gh,
                        ..
                    },
                ) => {
                    assert_eq!(fm, gm, "method flag {}", g.name);
                    assert_eq!(fh, gh, "fn header_span {}", g.name);
                    assert_eq!(fp.len(), gp.len(), "param count {}", g.name);
                    for (a, b) in fp.iter().zip(gp) {
                        assert_eq!(a.name, b.name);
                        assert_eq!(a.ty.as_ref().map(|t| t.span), b.ty.as_ref().map(|t| t.span));
                    }
                }
                (ExprKind::Class { .. }, ExprKind::Class { .. }) => {}
                (ExprKind::TypeAlias { .. }, ExprKind::TypeAlias { .. }) => {}
                (a, b) => assert_eq!(
                    std::mem::discriminant(a),
                    std::mem::discriminant(b),
                    "expr kind mismatch for {}",
                    g.name
                ),
            }
        }
    }

    #[test]
    fn ensure_base_full_metadata_lazily_populates_syntax_and_annotations() {
        let parsed = crate::parser::parse_module("x = 1\n").unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::with_context(program.clone(), EvaluationContext::default());
        vm.start(program.entry, vec![]).unwrap();
        let _module = ready(&mut vm).unwrap().as_object().unwrap();
        assert!(
            !vm.program.base_full_metadata_loaded,
            "cold evaluation without reflection/schema must not eagerly load full base.pkl AST"
        );
        vm.ensure_base_full_metadata().unwrap();
        assert!(vm.program.base_full_metadata_loaded);
        assert!(
            !vm.program.base_reflection_annotations_loaded,
            "ensure_base_full_metadata should defer compiling pkl:base reflection annotations"
        );
        let base_unit = vm
            .program
            .units
            .iter()
            .find(|u| u.standard_library && u.singleton_cache_key == Some("pkl:base"))
            .expect("base unit loaded");
        assert!(base_unit.syntax.is_some());
        vm.ensure_base_reflection_annotations().unwrap();
        assert!(vm.program.base_reflection_annotations_loaded);
        let has_base_annotations = vm
            .program
            .reflection_annotations
            .keys()
            .chain(
                vm.program
                    .base_reflection_annotations
                    .iter()
                    .flat_map(|m| m.keys()),
            )
            .any(|(uri, _)| uri.as_str() == Some("pkl:base"));
        assert!(
            has_base_annotations,
            "ensure_base_reflection_annotations must compile pkl:base reflection annotations"
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
