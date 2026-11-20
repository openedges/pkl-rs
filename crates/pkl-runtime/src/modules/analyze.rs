//! Static import loading core. No guest module compilation or evaluation occurs.
//! The VM binding projects only explicitly modeled failures; other causes remain typed.
mod collect;
use super::*;
pub use crate::host::resources::analysis_uri::UriFailure;
use crate::{diagnostic::Diagnostic, host::resources::analysis_uri::Uri, source::Span};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug)]
pub struct ImportGraph {
    /// Java URI component order, independently of Host enumeration order.
    pub nodes: Vec<ImportNode>,
    /// False when sibling recursion could depend on Java identity-hash order.
    /// Successful graph content is still complete; effect/error order is not qualified.
    pub traversal_order_qualified: bool,
}
#[derive(Clone, Debug)]
pub struct ImportNode {
    pub uri: PklString,
    pub resolved_uri: PklString,
    pub imports: Vec<PklString>,
}
#[derive(Clone, Debug)]
pub struct ImportSite {
    /// None for a parser failure with no imported URI.
    pub authored_uri: Option<PklString>,
    pub source: SourceText,
    pub source_uri: PklString,
    pub span: Span,
    /// The existing syntax model retains the complete parent clause span.
    /// Exact Java string-section projection for that clause is pending.
    pub parent_clause: bool,
}
#[derive(Clone, Debug)]
pub enum FailureCause {
    Host(ModuleHostError),
    Syntax(Box<Diagnostic>),
    Uri {
        input: PklString,
        reason: UriFailure,
    },
    Unsupported(PklString),
}
#[derive(Clone, Debug)]
pub struct AnalysisFailure {
    pub phase: &'static str,
    pub module_uri: PklString,
    pub site: Option<ImportSite>,
    pub cause: FailureCause,
    pub traversal_order_qualified: bool,
}
#[derive(Clone, Debug)]
pub enum AnalysisPoll {
    Request(Box<ModuleRequest>),
    Ready(Result<ImportGraph, Box<AnalysisFailure>>),
    Closed,
}
#[derive(Clone)]
struct Target {
    uri: Uri,
    key: ResolvedModuleKey,
}
struct Node {
    target: Target,
    source: SourceText,
    remaining: VecDeque<collect::Entry>,
    children: Vec<Target>,
    pending_glob_targets: VecDeque<Uri>,
}
enum Work {
    Root(Box<Uri>),
    Child(Box<Target>),
}
enum Action {
    Select {
        uri: Box<Uri>,
        parent: Option<Box<Node>>,
        entry: Option<collect::Entry>,
    },
    Resolve {
        uri: Box<Uri>,
        parent: Option<Box<Node>>,
        entry: Option<collect::Entry>,
    },
    Load(Box<Target>),
    ImportUri {
        node: Box<Node>,
        entry: collect::Entry,
    },
    ImportGlob {
        node: Box<Node>,
        entry: collect::Entry,
    },
    TripleDotCapabilities {
        node: Box<Node>,
        entry: collect::Entry,
        suffix: PklString,
    },
    TripleDotProbe {
        node: Box<Node>,
        entry: collect::Entry,
        candidate: PklString,
        remaining: VecDeque<PklString>,
    },
}
struct Pending {
    request: ModuleRequest,
    action: Action,
}

#[derive(Clone, Debug)]
pub struct PendingGlobInfo {
    pub base_uri: PklString,
    pub authored: PklString,
    pub pattern_uri: PklString,
    pub span: Span,
    pub target_key: ResolvedModuleKey,
}

struct ActiveGlob {
    node: Box<Node>,
    entry: collect::Entry,
}

/// Independently pollable shared loading continuation. Request IDs are local to
/// this owner. Hosts must return typed key identities, never evaluator Values.
/// Close abandons all pending keys/source buffers. No permanent cache is used.
pub struct ImportGraphAnalysis {
    work: VecDeque<Work>,
    nodes: BTreeMap<Uri, (PklString, Vec<PklString>)>,
    pending: Option<Pending>,
    pending_glob: Option<PendingGlobInfo>,
    active_glob: Option<ActiveGlob>,
    result: Option<Result<ImportGraph, Box<AnalysisFailure>>>,
    next_id: u64,
    order_qualified: bool,
    closed: bool,
}
impl ImportGraphAnalysis {
    pub fn new(roots: impl IntoIterator<Item = PklString>) -> Self {
        let mut this = Self {
            work: VecDeque::new(),
            nodes: BTreeMap::new(),
            pending: None,
            pending_glob: None,
            active_glob: None,
            result: None,
            next_id: 1,
            order_qualified: true,
            closed: false,
        };
        // All input URI validation precedes every factory/Host request.
        for root in roots {
            match Uri::parse(root.clone()) {
                Ok(uri) => this.work.push_back(Work::Root(Box::new(uri))),
                Err(reason) => {
                    this.fail(
                        "input URI",
                        root.clone(),
                        None,
                        FailureCause::Uri {
                            input: root,
                            reason,
                        },
                    );
                    break;
                }
            }
        }
        this
    }
    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("analysis request identity exhausted");
        id
    }
    fn emit(&mut self, kind: ModuleRequestKind, action: Action) {
        let id = self.id();
        self.pending = Some(Pending {
            request: ModuleRequest { id, kind },
            action,
        });
    }
    fn fail(
        &mut self,
        phase: &'static str,
        module_uri: PklString,
        site: Option<ImportSite>,
        cause: FailureCause,
    ) {
        self.pending = None;
        self.pending_glob = None;
        self.active_glob = None;
        self.work.clear();
        self.nodes.clear();
        self.result = Some(Err(Box::new(AnalysisFailure {
            phase,
            module_uri,
            site,
            cause,
            traversal_order_qualified: self.order_qualified,
        })));
    }
    fn site(node: &Node, entry: &collect::Entry) -> ImportSite {
        ImportSite {
            authored_uri: Some(entry.uri.clone()),
            source: node.source.clone(),
            source_uri: node.target.key.key.uri.clone(),
            span: entry.span,
            parent_clause: entry.parent_clause,
        }
    }
    fn select(&mut self, uri: Uri, parent: Option<Box<Node>>, entry: Option<collect::Entry>) {
        let phase = if parent.is_some() {
            SelectionPhase::RuntimeImport
        } else {
            SelectionPhase::Primary
        };
        let id = ModuleKeyId(self.id());
        self.emit(
            ModuleRequestKind::SelectKey {
                id,
                uri: uri.factory_uri(),
                phase,
            },
            Action::Select {
                uri: Box::new(uri),
                parent,
                entry,
            },
        );
    }
    fn load(&mut self, target: Target) {
        self.emit(
            ModuleRequestKind::LoadSource {
                key: target.key.clone(),
                resolved: target.key.module.clone(),
            },
            Action::Load(Box::new(target)),
        );
    }
    fn advance(&mut self, mut node: Box<Node>) {
        if let Some(uri) = node.pending_glob_targets.pop_front() {
            self.select(uri, Some(node), None);
            return;
        }
        if let Some(entry) = node.remaining.pop_front() {
            if entry.glob {
                if entry.uri.starts_with("...") {
                    self.fail(
                        "import glob",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Unsupported(
                            "Cannot combine glob imports with triple-dot module URIs.".into(),
                        ),
                    );
                    return;
                }
                self.emit(
                    ModuleRequestKind::ResolveImportUri {
                        key: node.target.key.key.clone(),
                        authored_uri: entry.uri.clone(),
                    },
                    Action::ImportGlob { node, entry },
                );
                return;
            }
            match crate::host::resources::import_triple_path(&entry.uri) {
                Ok(Some(suffix)) => {
                    let module = ModuleIdentity {
                        original_uri: node.target.key.module.original_uri.clone(),
                        resolved_uri: node.target.key.module.resolved_uri.clone(),
                    };
                    let key = Some(node.target.key.key.clone());
                    self.emit(
                        ModuleRequestKind::Capabilities { key, module },
                        Action::TripleDotCapabilities {
                            node,
                            entry,
                            suffix,
                        },
                    );
                    return;
                }
                Err(error) => {
                    self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Host(ModuleHostError::EvalError {
                            message: error.message.clone(),
                            hint: error.hint.clone(),
                        }),
                    );
                    return;
                }
                Ok(None) => {}
            }
            self.emit(
                ModuleRequestKind::ResolveImportUri {
                    key: node.target.key.key.clone(),
                    authored_uri: entry.uri.clone(),
                },
                Action::ImportUri { node, entry },
            );
        } else {
            let mut imports = BTreeSet::new();
            for target in &node.children {
                match Uri::parse(target.key.key.uri.clone()) {
                    Ok(uri) => {
                        imports.insert(uri);
                    }
                    Err(reason) => {
                        self.fail(
                            "original key URI",
                            target.key.key.uri.clone(),
                            None,
                            FailureCause::Uri {
                                input: target.key.key.uri.clone(),
                                reason,
                            },
                        );
                        return;
                    }
                }
            }
            self.nodes.insert(
                node.target.uri,
                (
                    node.target.key.module.resolved_uri.clone(),
                    imports.into_iter().map(|uri| uri.text).collect(),
                ),
            );
            // All direct imports have been resolved. This explicit source-order
            // descent admits successful branching graphs, but does not claim
            // Java HashSet identity-dependent sibling effect/error order.
            if node.children.len() > 1 {
                self.order_qualified = false;
            }
            for child in node.children.into_iter().rev() {
                self.work.push_front(Work::Child(Box::new(child)));
            }
        }
    }
    pub fn poll(&mut self) -> AnalysisPoll {
        if self.closed {
            return AnalysisPoll::Closed;
        }
        if let Some(result) = &self.result {
            return AnalysisPoll::Ready(result.clone());
        }
        if let Some(pending) = &self.pending {
            return AnalysisPoll::Request(Box::new(pending.request.clone()));
        }
        if let Some(active) = &self.active_glob {
            return AnalysisPoll::Ready(Err(Box::new(AnalysisFailure {
                phase: "import glob",
                module_uri: active.node.target.uri.text.clone(),
                site: Some(Self::site(&active.node, &active.entry)),
                cause: FailureCause::Unsupported(
                    "Static analyzer glob traversal requires host glob resolution.".into(),
                ),
                traversal_order_qualified: self.order_qualified,
            })));
        }
        while let Some(work) = self.work.pop_front() {
            match work {
                Work::Root(uri) => self.select(*uri, None, None),
                Work::Child(target) if !self.nodes.contains_key(&target.uri) => self.load(*target),
                Work::Child(_) => continue,
            }
            return AnalysisPoll::Request(Box::new(
                self.pending
                    .as_ref()
                    .expect("issued request")
                    .request
                    .clone(),
            ));
        }
        let nodes = std::mem::take(&mut self.nodes)
            .into_iter()
            .map(|(uri, (resolved_uri, imports))| ImportNode {
                uri: uri.text,
                resolved_uri,
                imports,
            })
            .collect();
        self.result = Some(Ok(ImportGraph {
            nodes,
            traversal_order_qualified: self.order_qualified,
        }));
        AnalysisPoll::Ready(self.result.as_ref().unwrap().clone())
    }
    /// A stale ID or wrong completion kind leaves the pending request intact.
    /// Typed Host failures become terminal structured failures, not guest errors.
    pub fn complete(&mut self, id: u64, completion: ModuleCompletion) -> Result<(), &'static str> {
        let pending = self.pending.as_ref().ok_or("no pending analysis request")?;
        if id != pending.request.id {
            return Err("stale analysis request");
        }
        match (&pending.request.kind, &completion) {
            // A selected key owns its absolute URI. Java ModuleResolver returns
            // the factory's key without replacing getUri() with its input;
            // the public VM validator imposes the same identity/URI boundary.
            (ModuleRequestKind::SelectKey { id, .. }, ModuleCompletion::KeySelected(Ok(key)))
                if key.id == *id
                    && crate::host::resources::validate_module_key_uri(&key.uri).is_ok()
                    && (key.kind != ModuleKeyKind::StandardLibrary
                        || stdlib_kind(&key.uri).is_some()) => {}
            (ModuleRequestKind::SelectKey { .. }, ModuleCompletion::KeySelected(Err(_))) => {}
            (
                ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                },
                ModuleCompletion::KeyResolved(Ok(resolved)),
            ) if resolved.id == *resolved_key_id && resolved.key == *key => {}
            (ModuleRequestKind::ResolveKey { .. }, ModuleCompletion::KeyResolved(Err(_))) => {}
            (ModuleRequestKind::LoadSource { .. }, ModuleCompletion::Source(_)) => {}
            (ModuleRequestKind::ResolveImportUri { .. }, ModuleCompletion::ImportUri(_)) => {}
            (ModuleRequestKind::Capabilities { .. }, ModuleCompletion::Capabilities(_)) => {}
            (ModuleRequestKind::HasElement { .. }, ModuleCompletion::HasElement(_)) => {}
            _ => return Err("analysis completion kind or key identity differs"),
        }
        let pending = self.pending.take().unwrap();
        match (pending.action, completion) {
            (Action::Select { uri, parent, entry }, ModuleCompletion::KeySelected(result)) => {
                match result {
                    Ok(key) => {
                        let resolved_key_id = ResolvedKeyId(self.id());
                        self.emit(
                            ModuleRequestKind::ResolveKey {
                                key: key.clone(),
                                resolved_key_id,
                                phase: ResolutionPhase::Query,
                            },
                            Action::Resolve { uri, parent, entry },
                        );
                    }
                    Err(error) => self.fail(
                        "factory",
                        uri.text,
                        parent
                            .as_ref()
                            .zip(entry.as_ref())
                            .map(|(p, e)| Self::site(p, e)),
                        FailureCause::Host(error),
                    ),
                }
            }
            (
                Action::Resolve {
                    uri,
                    mut parent,
                    entry,
                },
                ModuleCompletion::KeyResolved(result),
            ) => match result {
                Ok(resolved) => {
                    let target = Target {
                        uri: *uri,
                        key: resolved,
                    };
                    if let Some(mut node) = parent.take() {
                        node.children.push(target);
                        self.advance(node);
                    } else {
                        self.load(target);
                    }
                }
                Err(error) => self.fail(
                    "resolve",
                    uri.text,
                    parent
                        .as_ref()
                        .zip(entry.as_ref())
                        .map(|(p, e)| Self::site(p, e)),
                    FailureCause::Host(error),
                ),
            },
            (Action::Load(target), ModuleCompletion::Source(result)) => match result {
                Ok(source) => match collect::imports(&source.text) {
                    Ok(entries) => self.advance(Box::new(Node {
                        target: *target,
                        source: source.text,
                        remaining: entries.into(),
                        children: Vec::new(),
                        pending_glob_targets: VecDeque::new(),
                    })),
                    Err(error) => self.fail(
                        "parse",
                        target.uri.text,
                        Some(ImportSite {
                            authored_uri: None,
                            source: source.text,
                            source_uri: target.key.key.uri,
                            span: error.span,
                            parent_clause: false,
                        }),
                        FailureCause::Syntax(Box::new(error)),
                    ),
                },
                Err(error) => self.fail("load", target.uri.text, None, FailureCause::Host(error)),
            },
            (Action::ImportUri { node, entry }, ModuleCompletion::ImportUri(result)) => {
                match result {
                    Ok(text) => match Uri::parse(text.clone()) {
                        Ok(uri) => self.select(uri, Some(node), Some(entry)),
                        Err(reason) => self.fail(
                            "import URI",
                            node.target.uri.text.clone(),
                            Some(Self::site(&node, &entry)),
                            FailureCause::Uri {
                                input: text,
                                reason,
                            },
                        ),
                    },
                    Err(error) => self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Host(error),
                    ),
                }
            }
            (Action::ImportGlob { node, entry }, ModuleCompletion::ImportUri(result)) => {
                match result {
                    Ok(text) => {
                        let info = PendingGlobInfo {
                            base_uri: node.target.key.key.uri.clone(),
                            authored: entry.uri.clone(),
                            pattern_uri: text,
                            span: entry.span,
                            target_key: node.target.key.clone(),
                        };
                        self.pending_glob = Some(info);
                        self.active_glob = Some(ActiveGlob { node, entry });
                    }
                    Err(error) => self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Host(error),
                    ),
                }
            }
            (
                Action::TripleDotCapabilities {
                    node,
                    entry,
                    suffix,
                },
                ModuleCompletion::Capabilities(result),
            ) => match result {
                Ok(caps) => match crate::host::resources::triple_candidates(
                    &node.target.key.key.uri,
                    &suffix,
                    caps,
                ) {
                    Ok(candidates) => {
                        let mut remaining: VecDeque<PklString> = candidates.into();
                        if let Some(candidate) = remaining.pop_front() {
                            let module = ModuleIdentity {
                                original_uri: node.target.key.module.original_uri.clone(),
                                resolved_uri: node.target.key.module.resolved_uri.clone(),
                            };
                            let key = Some(node.target.key.key.clone());
                            self.emit(
                                ModuleRequestKind::HasElement {
                                    key,
                                    module,
                                    element_uri: candidate.clone(),
                                },
                                Action::TripleDotProbe {
                                    node,
                                    entry,
                                    candidate,
                                    remaining,
                                },
                            );
                        } else {
                            self.fail(
                                "import URI",
                                node.target.uri.text.clone(),
                                Some(Self::site(&node, &entry)),
                                FailureCause::Host(ModuleHostError::NotFound),
                            );
                        }
                    }
                    Err(error) => self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Host(ModuleHostError::EvalError {
                            message: error.message.clone(),
                            hint: error.hint.clone(),
                        }),
                    ),
                },
                Err(error) => self.fail(
                    "import URI",
                    node.target.uri.text.clone(),
                    Some(Self::site(&node, &entry)),
                    FailureCause::Host(error),
                ),
            },
            (
                Action::TripleDotProbe {
                    node,
                    entry,
                    candidate,
                    mut remaining,
                },
                ModuleCompletion::HasElement(result),
            ) => match result {
                Ok(true) => match Uri::parse(candidate.clone()) {
                    Ok(uri) => self.select(uri, Some(node), Some(entry)),
                    Err(reason) => self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        Some(Self::site(&node, &entry)),
                        FailureCause::Uri {
                            input: candidate,
                            reason,
                        },
                    ),
                },
                Ok(false) => {
                    if let Some(next_candidate) = remaining.pop_front() {
                        let module = ModuleIdentity {
                            original_uri: node.target.key.module.original_uri.clone(),
                            resolved_uri: node.target.key.module.resolved_uri.clone(),
                        };
                        let key = Some(node.target.key.key.clone());
                        self.emit(
                            ModuleRequestKind::HasElement {
                                key,
                                module,
                                element_uri: next_candidate.clone(),
                            },
                            Action::TripleDotProbe {
                                node,
                                entry,
                                candidate: next_candidate,
                                remaining,
                            },
                        );
                    } else {
                        self.fail(
                            "import URI",
                            node.target.uri.text.clone(),
                            Some(Self::site(&node, &entry)),
                            FailureCause::Host(ModuleHostError::NotFound),
                        );
                    }
                }
                Err(error) => self.fail(
                    "import URI",
                    node.target.uri.text.clone(),
                    Some(Self::site(&node, &entry)),
                    FailureCause::Host(error),
                ),
            },
            _ => unreachable!("completion validated before state change"),
        }
        Ok(())
    }
    pub fn is_pending_glob(&self) -> bool {
        self.pending_glob.is_some()
    }
    pub fn take_pending_glob(&mut self) -> Option<PendingGlobInfo> {
        self.pending_glob.take()
    }
    pub fn complete_glob(&mut self, matched_uris: Vec<PklString>) {
        let Some(ActiveGlob { mut node, entry: _ }) = self.active_glob.take() else {
            return;
        };
        for text in matched_uris {
            match Uri::parse(text.clone()) {
                Ok(uri) => node.pending_glob_targets.push_back(uri),
                Err(reason) => {
                    self.fail(
                        "import URI",
                        node.target.uri.text.clone(),
                        None,
                        FailureCause::Uri {
                            input: text,
                            reason,
                        },
                    );
                    return;
                }
            }
        }
        self.advance(node);
    }
    pub fn fail_glob(&mut self, cause: FailureCause) {
        let Some(ActiveGlob { node, entry }) = self.active_glob.take() else {
            return;
        };
        self.fail(
            "import glob",
            node.target.uri.text.clone(),
            Some(Self::site(&node, &entry)),
            cause,
        );
    }
    pub fn close(&mut self) {
        self.closed = true;
        self.pending = None;
        self.pending_glob = None;
        self.active_glob = None;
        self.result = None;
        self.work.clear();
        self.nodes.clear();
    }
}
