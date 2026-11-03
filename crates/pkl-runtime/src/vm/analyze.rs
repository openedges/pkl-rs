//! pkl:analyze uses a static loading continuation, never an imported guest VM.
//! The authored module/classes still use ordinary shared-engine identities.
use super::*;
use crate::heap::CollectionKind;
use crate::modules::analyze::{
    AnalysisFailure, AnalysisPoll, FailureCause, ImportGraphAnalysis, ImportNode,
};
use crate::modules::{
    ModuleCompletion, ModuleKey, ModuleKeyId, ModuleRequest, ModuleRequestKind, ResolvedKeyId,
    ResolvedModuleKey,
};
use std::collections::VecDeque;

#[derive(Clone, Copy)]
enum Stage {
    Analyze,
    Module,
    GraphClass,
    ImportClass,
    Node,
    Edge,
    HashedEdge,
}
pub(super) struct Run {
    analysis: Option<ImportGraphAnalysis>,
    pending: Option<ModuleRequest>,
    // Private core IDs must not collide with another import/query/custom Host.
    keys: BTreeMap<ModuleKeyId, ModuleKeyId>,
    resolved: BTreeMap<ResolvedKeyId, ResolvedKeyId>,
    stage: Stage,
    nodes: VecDeque<ImportNode>,
    node: Option<ImportNode>,
    edge: usize,
    module: Option<ObjectId>,
    graph_class: Option<ObjectId>,
    import_class: Option<ObjectId>,
    imports: Option<ObjectId>,
    resolved_imports: Option<ObjectId>,
    edges: Option<ObjectId>,
    edge_value: Option<Value>,
}
impl Run {
    fn new(roots: Vec<PklString>) -> Self {
        Self {
            analysis: Some(ImportGraphAnalysis::new(roots)),
            pending: None,
            keys: BTreeMap::new(),
            resolved: BTreeMap::new(),
            stage: Stage::Analyze,
            nodes: VecDeque::new(),
            node: None,
            edge: 0,
            module: None,
            graph_class: None,
            import_class: None,
            imports: None,
            resolved_imports: None,
            edges: None,
            edge_value: None,
        }
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(
            self.module
                .into_iter()
                .chain(self.imports)
                .chain(self.resolved_imports)
                .chain(self.edges)
                .map(Value::Object),
        );
        roots.extend(
            self.graph_class
                .into_iter()
                .chain(self.import_class)
                .map(Value::Class),
        );
        roots.extend(self.edge_value.iter().cloned());
    }
    pub(super) fn awaiting_host(&self) -> bool {
        self.pending.is_some()
    }
    fn key(&self, key: &ModuleKey) -> Result<ModuleKey, RuntimeError> {
        let mut result = key.clone();
        result.id = *self
            .keys
            .get(&key.id)
            .ok_or_else(|| invalid("analysis key mapping"))?;
        Ok(result)
    }
    fn request(&mut self, request: ModuleRequest) -> Result<ModuleRequestKind, RuntimeError> {
        let kind = match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, phase } => {
                let global = ModuleKeyId(modules::next_key_id()?);
                self.keys.insert(*id, global);
                ModuleRequestKind::SelectKey {
                    id: global,
                    uri: uri.clone(),
                    phase: *phase,
                }
            }
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                phase,
            } => {
                let global = ResolvedKeyId(modules::next_key_id()?);
                self.resolved.insert(*resolved_key_id, global);
                ModuleRequestKind::ResolveKey {
                    key: self.key(key)?,
                    resolved_key_id: global,
                    phase: *phase,
                }
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                ModuleRequestKind::ResolveImportUri {
                    key: self.key(key)?,
                    authored_uri: authored_uri.clone(),
                }
            }
            ModuleRequestKind::LoadSource { key, resolved } => {
                let mut global = key.clone();
                global.key = self.key(&key.key)?;
                global.id = *self
                    .resolved
                    .get(&key.id)
                    .ok_or_else(|| invalid("analysis resolved key mapping"))?;
                ModuleRequestKind::LoadSource {
                    key: global,
                    resolved: resolved.clone(),
                }
            }
            _ => return Err(invalid("unmodeled static analysis request")),
        };
        self.pending = Some(request);
        Ok(kind)
    }
    fn complete(&mut self, completion: ModuleCompletion) -> Result<(), RuntimeError> {
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| invalid("analysis completion without request"))?;
        let completion = match (&request.kind, completion) {
            (ModuleRequestKind::SelectKey { id, .. }, ModuleCompletion::KeySelected(result)) => {
                ModuleCompletion::KeySelected(result.map(|mut key| {
                    key.id = *id;
                    key
                }))
            }
            (
                ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                },
                ModuleCompletion::KeyResolved(result),
            ) => ModuleCompletion::KeyResolved(result.map(|mut resolved| {
                resolved.id = *resolved_key_id;
                resolved.key = key.clone();
                resolved
            })),
            // The legacy Host profile carries the same resolved identity minus
            // its opaque resolved-key handle; match ordinary module completion.
            (
                ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                },
                ModuleCompletion::Resolved(result),
            ) => ModuleCompletion::KeyResolved(result.map(|module| ResolvedModuleKey {
                id: *resolved_key_id,
                key: key.clone(),
                handle: "".into(),
                module,
            })),
            (_, completion) => completion,
        };
        self.analysis
            .as_mut()
            .ok_or_else(|| invalid("closed analysis completion"))?
            .complete(request.id, completion)
            .map_err(invalid)?;
        self.pending = None;
        Ok(())
    }
}
impl Vm {
    pub(super) fn begin_analysis(&mut self) -> Result<(), RuntimeError> {
        let argument = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .ok_or_else(|| invalid("analysis argument"))?;
        let id = argument.as_object()?;
        if self.heap.get(id)?.collection_kind != CollectionKind::Set {
            return Err(invalid(
                "analysis argument has not passed Set<Uri> checking",
            ));
        }
        let mut roots = Vec::new();
        for member in collections::ordered_members(&self.heap, id)? {
            let value = collections::cached_render_value(&self.heap, id, &member)?
                .ok_or_else(|| invalid("analysis strict Set element"))?;
            let Value::String(uri) = value else {
                return Err(invalid(
                    "analysis argument has not passed Uri element checking",
                ));
            };
            roots.push(uri);
        }
        self.frames.last_mut().unwrap().analysis = Some(Box::new(Run::new(roots)));
        Ok(())
    }
    pub(super) fn complete_analysis_module(
        &mut self,
        completion: ModuleCompletion,
    ) -> Result<(), RuntimeError> {
        self.frames
            .last_mut()
            .and_then(|frame| frame.analysis.as_mut())
            .ok_or_else(|| invalid("missing analysis continuation"))?
            .complete(completion)?;
        self.state = State::Running;
        Ok(())
    }
    fn analysis_object(
        &mut self,
        class: ObjectId,
        fields: Vec<(&str, Value)>,
    ) -> Result<ObjectId, RuntimeError> {
        let prototype = self.heap.get_class(class)?.prototype;
        let mut object = Object {
            parent: Some(prototype),
            instance_class: Some(class),
            ..Object::default()
        };
        for (name, value) in fields {
            let name: PklString = name.into();
            object.members.push(Member {
                flags: 0,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                entry: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            // Owner values remain available when an ordinary amendment inherits
            // the factory member. These edges are already traced by Heap.
            object.member_values.insert(
                CacheKey {
                    owner: None,
                    name,
                    method: false,
                    entry: false,
                },
                value,
            );
        }
        Ok(self.heap.allocate(object))
    }
    pub(super) fn step_analysis(&mut self) -> Result<(), RuntimeError> {
        let index = self.frames.len() - 1;
        let mut run = self.frames[index]
            .analysis
            .take()
            .ok_or_else(|| invalid("missing analysis continuation"))?;
        match run.stage {
            Stage::Analyze => match run
                .analysis
                .as_mut()
                .ok_or_else(|| invalid("analysis core"))?
                .poll()
            {
                AnalysisPoll::Request(request) => {
                    let kind = run.request(*request)?;
                    self.frames[index].analysis = Some(run);
                    return self.wait_module(kind);
                }
                AnalysisPoll::Ready(Err(error)) => {
                    return Err(analysis_error(
                        *error,
                        self.program.metadata.get(self.frames[index].block),
                    ));
                }
                AnalysisPoll::Ready(Ok(graph)) => {
                    run.nodes = graph.nodes.into();
                    run.analysis = None;
                    run.keys.clear();
                    run.resolved.clear();
                    run.stage = Stage::Module;
                    self.frames[index].analysis = Some(run);
                    // Resolve canonical factory classes, not a virtual class on
                    // a borrowed method receiver. Only this stdlib is evaluated.
                    self.import_stdlib("pkl:analyze")?;
                    return Ok(());
                }
                AnalysisPoll::Closed => return Err(invalid("closed analysis resumed")),
            },
            Stage::Module => {
                let module = self.pop()?.as_object()?;
                run.module = Some(module);
                run.stage = Stage::GraphClass;
                self.frames[index].analysis = Some(run);
                return self.force(module, module, "ImportGraph".into(), false, true, false);
            }
            Stage::GraphClass => {
                let Value::Class(class) = self.pop()? else {
                    return Err(invalid("analysis graph class"));
                };
                run.graph_class = Some(class);
                run.stage = Stage::ImportClass;
                let module = run.module.ok_or_else(|| invalid("analysis module"))?;
                self.frames[index].analysis = Some(run);
                return self.force(module, module, "Import".into(), false, true, false);
            }
            Stage::ImportClass => {
                let Value::Class(class) = self.pop()? else {
                    return Err(invalid("analysis import class"));
                };
                run.import_class = Some(class);
                run.imports = Some(self.heap.allocate(Object {
                    collection_kind: CollectionKind::Map,
                    ..Object::default()
                }));
                run.resolved_imports = Some(self.heap.allocate(Object {
                    collection_kind: CollectionKind::Map,
                    ..Object::default()
                }));
                run.stage = Stage::Node;
            }
            Stage::Node => {
                if let Some(node) = run.nodes.pop_front() {
                    run.node = Some(node);
                    run.edge = 0;
                    run.edges = Some(self.heap.allocate(Object {
                        collection_kind: CollectionKind::Set,
                        ..Object::default()
                    }));
                    run.stage = Stage::Edge;
                } else {
                    let imports = self.finish_strict_collection(
                        run.imports.ok_or_else(|| invalid("analysis imports map"))?,
                    )?;
                    let resolved = self.finish_strict_collection(
                        run.resolved_imports
                            .ok_or_else(|| invalid("analysis resolved map"))?,
                    )?;
                    let result = self.analysis_object(
                        run.graph_class
                            .ok_or_else(|| invalid("analysis graph class"))?,
                        vec![
                            ("imports", Value::Object(imports)),
                            ("resolvedImports", Value::Object(resolved)),
                        ],
                    )?;
                    self.stack.push(Value::Object(result));
                    return Ok(());
                }
            }
            Stage::Edge => {
                let node = run.node.as_ref().ok_or_else(|| invalid("analysis node"))?;
                if let Some(uri) = node.imports.get(run.edge).cloned() {
                    let value = Value::Object(
                        self.analysis_object(
                            run.import_class
                                .ok_or_else(|| invalid("analysis import class"))?,
                            vec![("uri", Value::String(uri))],
                        )?,
                    );
                    run.edge_value = Some(value.clone());
                    run.stage = Stage::HashedEdge;
                    self.frames[index].analysis = Some(run);
                    // The ordinary structural-key continuation handles typed
                    // factory values and roots them across poll/collection.
                    self.prepare_render_key(value);
                    return Ok(());
                }
                let set = self.finish_strict_collection(
                    run.edges.ok_or_else(|| invalid("analysis edge set"))?,
                )?;
                self.insert_map_entry_ordered(
                    run.imports.ok_or_else(|| invalid("analysis imports map"))?,
                    Value::String(node.uri.clone()),
                    Value::Object(set),
                    Some(true),
                )?;
                self.insert_map_entry_ordered(
                    run.resolved_imports
                        .ok_or_else(|| invalid("analysis resolved map"))?,
                    Value::String(node.uri.clone()),
                    Value::String(node.resolved_uri.clone()),
                    Some(true),
                )?;
                run.node = None;
                run.edges = None;
                run.stage = Stage::Node;
            }
            Stage::HashedEdge => {
                let set = run.edges.ok_or_else(|| invalid("analysis edge set"))?;
                let value = run
                    .edge_value
                    .take()
                    .ok_or_else(|| invalid("analysis edge value"))?;
                let mut hashes = self
                    .heap
                    .get(set)?
                    .set_entries
                    .iter()
                    .map(|v| self.java_hash(v))
                    .collect::<Result<Vec<_>, _>>()?;
                hashes.push(self.java_hash(&value)?);
                let object = self.heap.get_mut(set)?;
                // Core edges are unique by URI.compareTo, hence their URI strings
                // and typed Import values are distinct. Still use the ordinary
                // hash trie and preserve the URI-sorted visible insertion order.
                object
                    .map_index
                    .get_or_insert_with(Box::default)
                    .insert(hashes, true);
                object.set_entries.push(value.clone());
                self.frames[index].builders.push(set);
                self.stack.push(value);
                self.append_strict_element()?;
                self.frames[index].builders.pop();
                run.edge += 1;
                run.stage = Stage::Edge;
            }
        }
        self.frames[index].analysis = Some(run);
        Ok(())
    }
}
fn analysis_error(failure: AnalysisFailure, root: Option<&BlockMetadata>) -> RuntimeError {
    use crate::modules::analyze::UriFailure;
    let mut error = match &failure.cause {
        FailureCause::Uri {
            input,
            reason: UriFailure::Relative,
        } if failure.phase == "input URI" => RuntimeError::new(
            ErrorKind::Type,
            PklString::from("Cannot analyze relative module URI `")
                .concat(input)
                .concat(&"`.".into()),
        ),
        FailureCause::Uri {
            input,
            reason:
                UriFailure::Syntax {
                    reason,
                    utf16_index: Some(index),
                },
        } if failure.phase == "input URI" => {
            let mut error = RuntimeError::host_language(
                PklString::from("Module URI `")
                    .concat(input)
                    .concat(&"` has invalid syntax.".into()),
            );
            // AnalyzeNodes uses URISyntaxException.getMessage(), unlike the
            // imported-URI branch which uses getReason(). Keep the original units.
            error.hint = Some(
                reason
                    .concat(&format!(" at index {index}: ").into())
                    .concat(input),
            );
            error
        }
        FailureCause::Host(crate::modules::ModuleHostError::NotFound)
            if matches!(failure.phase, "resolve" | "import URI")
                && failure
                    .site
                    .as_ref()
                    .is_some_and(|site| !site.parent_clause && site.authored_uri.is_some())
                && root.is_some_and(|root| root.source.is_some()) =>
        {
            let site = failure.site.as_ref().expect("checked import site");
            let root = root.expect("checked analyzer root");
            let mut error = crate::modules::ModuleHostError::NotFound.into_runtime(
                site.authored_uri
                    .as_ref()
                    .expect("checked authored URI")
                    .clone(),
            );
            // StackTraceGenerator replaces the external method's source section
            // with the imported string section, while retaining its actual root
            // name. The imported module is parsed only; it has no guest frame.
            error.frames.push(RuntimeFrame {
                anonymous: root.anonymous_root,
                member: root.member.clone(),
                span: site.span,
                source: Some(Arc::new(crate::value::RuntimeSource {
                    text: site.source.clone(),
                    uri: site.source_uri.clone(),
                    module_name: root
                        .source
                        .as_ref()
                        .expect("checked root source")
                        .module_name
                        .clone(),
                })),
            });
            error
        }
        _ => RuntimeError::new(
            ErrorKind::Unsupported,
            format!(
                "Static import analysis {} failure requires Java diagnostic projection.",
                failure.phase
            ),
        ),
    };
    // Root URI failures have an unavailable external origin. Imported failures
    // above already replace that origin with their explicit source section.
    error.unavailable_origin = true;
    error
}

#[cfg(test)]
mod diagnostic_tests {
    // These are source-derived state/provenance controls, not Java observations
    // or raw API compatibility expectations.
    use super::*;
    use crate::modules::{ModuleHostError, ModuleKeyKind, ModuleSource, ResolvedModule};
    use crate::source::SourceText;
    use crate::value::RuntimeSource;

    fn analyzer_root() -> BlockMetadata {
        BlockMetadata {
            source: Some(Arc::new(RuntimeSource {
                text: SourceText::from(
                    "external function importGraph(moduleUris: Set<String>): ImportGraph",
                ),
                uri: "pkl:analyze".into(),
                module_name: "pkl.analyze".into(),
            })),
            member: "importGraph".into(),
            ..BlockMetadata::default()
        }
    }
    fn preflight(input: &str) -> AnalysisFailure {
        let mut run = ImportGraphAnalysis::new([
            "file:///must-not-be-requested.pkl".into(),
            input.into(),
            "file:///later.pkl".into(),
        ]);
        let AnalysisPoll::Ready(Err(failure)) = run.poll() else {
            panic!("all URI validation must finish before any Host request")
        };
        run.close();
        assert!(matches!(run.poll(), AnalysisPoll::Closed));
        *failure
    }
    #[test]
    fn root_syntax_projection_retains_utf16_indices_and_unescaped_hint_input() {
        for (input, reason, index) in [
            ("foo <>", "Illegal character in path", 3),
            ("𝄞 <>", "Illegal character in path", 2),
            ("x:", "Expected scheme-specific part", 2),
            ("file:/𝄞%zz", "Malformed escape pair", 8),
            ("file:///𝄞?a%", "Malformed escape pair", 12),
            ("file:/x#𝄞 ", "Illegal character in fragment", 10),
            ("opaque:𝄞%x", "Malformed escape pair", 9),
            ("a%xx/b:c%yy", "Malformed escape pair", 1),
        ] {
            let failure = preflight(input);
            let error = analysis_error(failure, Some(&analyzer_root()));
            assert_eq!(error.kind, ErrorKind::Host);
            assert!(error.host_language_exception);
            assert_eq!(
                error.message,
                PklString::from(format!("Module URI `{input}` has invalid syntax."))
            );
            assert_eq!(
                error.hint,
                Some(format!("{reason} at index {index}: {input}").into())
            );
            assert!(error.frames.is_empty());
            assert!(error.unavailable_origin);
        }
    }
    #[test]
    fn relative_and_unmodeled_uri_failures_are_distinct_before_host_loading() {
        let relative = analysis_error(preflight("foo.pkl"), Some(&analyzer_root()));
        assert_eq!(relative.kind, ErrorKind::Type);
        assert_eq!(
            relative.message,
            "Cannot analyze relative module URI `foo.pkl`."
        );
        assert_eq!(relative.hint, None);
        let scoped = preflight("http://[fe80::1%eth0]/");
        assert!(matches!(
            scoped.cause,
            FailureCause::Uri {
                reason: crate::modules::analyze::UriFailure::Unsupported(_),
                ..
            }
        ));
        assert_eq!(
            analysis_error(scoped, Some(&analyzer_root())).kind,
            ErrorKind::Unsupported
        );
        // This parser diagnoses malformed authority escapes but does not yet
        // promise Java's server-vs-registry fallback index.
        let authority = analysis_error(preflight("http://bad%xx/x"), Some(&analyzer_root()));
        assert_eq!(authority.kind, ErrorKind::Unsupported);
    }

    fn loading_failure(source: &str, missing_at: &str) -> (AnalysisFailure, Vec<String>) {
        let mut run = ImportGraphAnalysis::new(["file:///requested.pkl".into()]);
        let mut events = Vec::new();
        for _ in 0..30 {
            let request = match run.poll() {
                AnalysisPoll::Ready(Err(failure)) => {
                    run.close();
                    assert!(matches!(run.poll(), AnalysisPoll::Closed));
                    return (*failure, events);
                }
                AnalysisPoll::Request(request) => request,
                other => panic!("expected typed analysis failure, got {other:?}"),
            };
            let completion = match &request.kind {
                ModuleRequestKind::SelectKey { id, uri, .. } => {
                    events.push(format!("select:{uri}"));
                    if (uri != "file:///requested.pkl" && missing_at == "factory")
                        || missing_at == "root factory"
                    {
                        ModuleCompletion::KeySelected(Err(ModuleHostError::NotFound))
                    } else {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            // The factory owns the original key identity. Its
                            // URI differs from both the request and read URI.
                            uri: if uri == "file:///requested.pkl" {
                                "file:///provider/main.pkl".into()
                            } else {
                                uri.clone()
                            },
                            kind: ModuleKeyKind::Host,
                            handle: "provider-owned-key".into(),
                        }))
                    }
                }
                ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                } => {
                    events.push(format!("resolve:{}", key.uri));
                    if key.uri != "file:///provider/main.pkl" && missing_at == "resolve" {
                        ModuleCompletion::KeyResolved(Err(ModuleHostError::NotFound))
                    } else {
                        ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: "provider-owned-read".into(),
                            module: ResolvedModule {
                                original_uri: key.uri.clone(),
                                resolved_uri: "file:///transport/source.pkl".into(),
                                source_uri: "file:///diagnostic-alias.pkl".into(),
                            },
                        }))
                    }
                }
                ModuleRequestKind::LoadSource { key, .. } => {
                    events.push(format!("load:{}", key.key.uri));
                    ModuleCompletion::Source(if missing_at == "load" {
                        Err(ModuleHostError::NotFound)
                    } else {
                        Ok(ModuleSource {
                            text: source.into(),
                            inferred_name: "ignored-loaded-name".into(),
                        })
                    })
                }
                ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                    events.push(format!("uri:{}:{authored_uri}", key.uri));
                    assert_eq!(key.uri, "file:///provider/main.pkl");
                    assert_eq!(authored_uri, "absent.pkl");
                    if missing_at == "import URI" {
                        ModuleCompletion::ImportUri(Err(ModuleHostError::NotFound))
                    } else {
                        crate::modules::default_completion(&request).unwrap()
                    }
                }
                other => panic!("unexpected static capability {other:?}"),
            };
            run.complete(request.id, completion).unwrap();
        }
        panic!("bounded static control did not finish")
    }
    #[test]
    fn missing_import_keeps_authored_uri_original_key_source_and_active_analyzer_root() {
        let source = "import \"absent.pkl\"\nimport \"later.pkl\"\nx=throw(\"not evaluated\")\n";
        for phase in ["import URI", "resolve"] {
            // Closing a failed continuation must not leak source/key state into
            // a subsequent independent analysis.
            for _ in 0..2 {
                let (failure, events) = loading_failure(source, phase);
                assert_eq!(failure.phase, phase);
                assert!(!events.iter().any(|event| event.contains("later.pkl")));
                let error = analysis_error(failure, Some(&analyzer_root()));
                assert_eq!(error.message, "Cannot find module `absent.pkl`.");
                assert!(error.host_language_exception);
                assert_eq!(error.hint, None);
                assert_eq!(error.frames.len(), 1);
                let frame = &error.frames[0];
                assert!(!frame.anonymous);
                assert_eq!(frame.member, "importGraph");
                let retained = frame.source.as_ref().unwrap();
                assert_eq!(retained.uri, "file:///provider/main.pkl");
                assert_eq!(retained.module_name, "pkl.analyze");
                assert_eq!(retained.text, SourceText::from(source));
                assert_eq!(
                    retained.text.slice(frame.span).unwrap().to_pkl_string(),
                    "\"absent.pkl\""
                );
            }
        }
    }
    #[test]
    fn factory_missing_retains_its_typed_site_but_is_not_an_import_missing_error() {
        // ModuleResolver wraps factory.create IOException before the analyzer's
        // FileNotFoundException catch. Its ioErrorLoadingModule/cause projection
        // is still pending, even when the original failure is FileNotFound.
        let (failure, events) =
            loading_failure("import \"absent.pkl\"\nimport \"later.pkl\"\n", "factory");
        assert_eq!(failure.phase, "factory");
        assert!(matches!(
            &failure.cause,
            FailureCause::Host(ModuleHostError::NotFound)
        ));
        assert_eq!(
            failure.site.as_ref().unwrap().authored_uri,
            Some("absent.pkl".into())
        );
        assert_eq!(events.last().unwrap(), "select:file:///provider/absent.pkl");
        assert!(!events.iter().any(|event| event.contains("later.pkl")));
        let error = analysis_error(failure, Some(&analyzer_root()));
        assert_eq!(error.kind, ErrorKind::Unsupported);
        assert!(!error.host_language_exception);
        assert_eq!(error.hint, None);
        assert!(error.frames.is_empty());
    }
    #[test]
    fn root_and_load_missing_are_not_import_site_errors() {
        for phase in ["root factory", "load"] {
            let (failure, _) = loading_failure("x=1", phase);
            assert!(failure.site.is_none());
            let error = analysis_error(failure, Some(&analyzer_root()));
            assert_eq!(error.kind, ErrorKind::Unsupported);
            assert!(error.frames.is_empty());
        }
    }
    #[test]
    fn malformed_module_precedes_import_callbacks_and_retains_original_source_identity() {
        let source = "import \"absent.pkl\"\nx=\n";
        let (failure, events) = loading_failure(source, "resolve");
        assert_eq!(failure.phase, "parse");
        assert!(!events.iter().any(|event| event.starts_with("uri:")));
        let site = failure.site.as_ref().unwrap();
        assert_eq!(site.authored_uri, None);
        assert_eq!(site.source_uri, "file:///provider/main.pkl");
        assert_eq!(site.source, SourceText::from(source));
        assert_eq!(
            analysis_error(failure, Some(&analyzer_root())).kind,
            ErrorKind::Unsupported
        );
    }
    #[test]
    fn parent_clause_span_gap_remains_explicit() {
        let (failure, _) = loading_failure("amends \"absent.pkl\"\n", "import URI");
        assert!(failure.site.as_ref().unwrap().parent_clause);
        assert_eq!(
            analysis_error(failure, Some(&analyzer_root())).kind,
            ErrorKind::Unsupported
        );
    }
}
