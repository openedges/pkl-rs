//! Fixture objects and getter snapshots. No evaluation or module cache lives here.
use pkl_runtime::modules::*;
use pkl_runtime::value::PklString;
use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

struct Config {
    requested: PklString,
    selected: PklString,
    resolved: PklString,
    source: String,
    cached: String,
    fragments: bool,
    source_mode: String,
    kind: ModuleKeyKind,
}
struct Selected {
    config: usize,
    cached: bool,
    source: String,
}
#[derive(Clone)]
pub struct Profile {
    pub uri: String,
    pub contents: Option<Arc<str>>,
}
pub struct Factory {
    configs: Vec<Config>,
    sources: BTreeMap<String, Result<Arc<str>, ModuleHostError>>,
    profiles: BTreeMap<String, Profile>,
    overrides: BTreeMap<(PklString, PklString), PklString>,
    capabilities: BTreeMap<PklString, ModuleCapabilities>,
    elements: BTreeMap<(PklString, PklString), bool>,
    element_defaults: BTreeMap<PklString, bool>,
    keys: BTreeMap<ModuleKeyId, Selected>,
    resolved: BTreeMap<ResolvedKeyId, ModuleKeyId>,
    pub cached: bool,
    pub custom: bool,
    state: String,
}
fn boolean(value: &str) -> bool {
    match value {
        "true" => true,
        "false" => false,
        _ => panic!("boolean configuration"),
    }
}
impl Factory {
    pub fn new(path: &Path) -> Self {
        let mut this = Self {
            configs: Vec::new(),
            sources: BTreeMap::new(),
            profiles: BTreeMap::new(),
            overrides: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            elements: BTreeMap::new(),
            element_defaults: BTreeMap::new(),
            keys: BTreeMap::new(),
            resolved: BTreeMap::new(),
            cached: true,
            custom: true,
            state: "good".into(),
        };
        let input = fs::read_to_string(path).expect("module factory configuration");
        let rows = input
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
        for columns in &rows {
            if columns[0] == "SOURCE" {
                assert_eq!(columns.len(), 4);
                let result = match columns[2] {
                    "text" => Ok(fs::read_to_string(columns[3])
                        .expect("configured source")
                        .into()),
                    "io" => Err(ModuleHostError::Io(pkl_runtime::host::IoFailure {
                        message: Some(super::utf16(columns[3])),
                    })),
                    _ => panic!("source result kind"),
                };
                assert!(
                    this.sources.insert(columns[1].into(), result).is_none(),
                    "duplicate source ID"
                );
            }
        }
        for columns in rows {
            match columns[0] {
                "SOURCE" => {}
                "KEY" => {
                    assert_eq!(columns.len(), 9);
                    assert!(matches!(columns[5], "true" | "false" | "dynamic"));
                    assert!(matches!(
                        columns[7],
                        "fixed" | "state-at-load" | "state-at-factory"
                    ));
                    this.configs.push(Config {
                        requested: columns[1].into(),
                        selected: columns[2].into(),
                        resolved: columns[3].into(),
                        source: columns[4].into(),
                        cached: columns[5].into(),
                        fragments: boolean(columns[6]),
                        source_mode: columns[7].into(),
                        kind: match columns[8] {
                            "host" => ModuleKeyKind::Host,
                            "synthetic" => ModuleKeyKind::Synthetic,
                            _ => panic!("configured actual key kind"),
                        },
                    });
                }
                "PROFILE" => {
                    assert_eq!(columns.len(), 5);
                    let contents = match columns[2] {
                        "uri" => {
                            assert!(columns[4].is_empty());
                            None
                        }
                        "contents" | "repl" => Some(
                            this.sources
                                .get(columns[4])
                                .expect("profile source ID")
                                .as_ref()
                                .expect("profile source text")
                                .clone(),
                        ),
                        _ => panic!("ModuleSource profile kind"),
                    };
                    assert!(
                        this.profiles
                            .insert(
                                columns[1].into(),
                                Profile {
                                    uri: columns[3].into(),
                                    contents
                                }
                            )
                            .is_none()
                    );
                }
                "OVERRIDE" => {
                    assert_eq!(columns.len(), 4);
                    assert!(
                        this.overrides
                            .insert((columns[1].into(), columns[2].into()), columns[3].into())
                            .is_none()
                    );
                }
                "CAPS" => {
                    assert_eq!(columns.len(), 5);
                    assert!(
                        this.capabilities
                            .insert(
                                columns[1].into(),
                                ModuleCapabilities {
                                    local: boolean(columns[2]),
                                    hierarchical: boolean(columns[3]),
                                    fragment_paths: boolean(columns[4])
                                }
                            )
                            .is_none()
                    );
                }
                "ELEMENT" => {
                    assert_eq!(columns.len(), 4);
                    assert!(
                        this.elements
                            .insert((columns[1].into(), columns[2].into()), boolean(columns[3]))
                            .is_none()
                    );
                }
                "ELEMENT_DEFAULT" => {
                    assert_eq!(columns.len(), 3);
                    assert!(
                        this.element_defaults
                            .insert(columns[1].into(), boolean(columns[2]))
                            .is_none()
                    );
                }
                _ => {
                    assert_eq!(columns.len(), 5, "legacy module factory columns");
                    let source = format!("legacy-{}", this.configs.len());
                    this.sources.insert(
                        source.clone(),
                        Ok(fs::read_to_string(columns[3])
                            .expect("configured fixture source")
                            .into()),
                    );
                    this.configs.push(Config {
                        requested: columns[0].into(),
                        selected: columns[1].into(),
                        resolved: columns[2].into(),
                        source,
                        cached: columns[4].into(),
                        fragments: false,
                        source_mode: "fixed".into(),
                        kind: ModuleKeyKind::Host,
                    });
                }
            }
        }
        this
    }
    pub fn set_source(&mut self, state: &str) {
        assert!(self.sources.contains_key(state), "undefined source state");
        self.state = state.into();
    }
    pub fn profile(&self, id: &str) -> Profile {
        self.profiles
            .get(id)
            .expect("configured ModuleSource profile")
            .clone()
    }
    fn selected(&self, key: &ModuleKey) -> Option<&Selected> {
        let selected = self.keys.get(&key.id)?;
        let config = &self.configs[selected.config];
        assert_eq!(key.handle, format!("custom-key-{}", key.id.0).as_str());
        assert_eq!(key.uri, config.selected);
        assert_eq!(key.kind, config.kind);
        Some(selected)
    }
    pub fn complete(&mut self, request: &ModuleRequest) -> Option<ModuleCompletion> {
        Some(match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                if !self.custom {
                    return None;
                }
                if uri.starts_with("throwing:") {
                    return Some(ModuleCompletion::KeySelected(Err(ModuleHostError::Io(
                        "stage48-factory-create".into(),
                    ))));
                }
                let index = self
                    .configs
                    .iter()
                    .position(|config| config.requested == *uri)?;
                let config = &self.configs[index];
                let cached = match config.cached.as_str() {
                    "true" => true,
                    "false" => false,
                    "dynamic" => self.cached,
                    _ => panic!("fixture cache mode"),
                };
                let source = if config.source_mode == "state-at-factory" {
                    self.state.clone()
                } else {
                    config.source.clone()
                };
                if config.source_mode != "state-at-load" {
                    assert!(
                        self.sources.contains_key(&source),
                        "undefined selected source"
                    );
                }
                self.keys.insert(
                    *id,
                    Selected {
                        config: index,
                        cached,
                        source,
                    },
                );
                ModuleCompletion::KeySelected(Ok(ModuleKey {
                    id: *id,
                    uri: config.selected.clone(),
                    kind: config.kind,
                    handle: format!("custom-key-{}", id.0).into(),
                }))
            }
            ModuleRequestKind::QueryCachePolicy { key } => {
                ModuleCompletion::CachePolicy(Ok(self.selected(key)?.cached))
            }
            ModuleRequestKind::QueryFragmentPaths { key } => ModuleCompletion::FragmentPaths(Ok(
                self.configs[self.selected(key)?.config].fragments,
            )),
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => {
                self.selected(key)?;
                ModuleCompletion::ImportUri(Ok(self
                    .overrides
                    .get(&(key.uri.clone(), authored_uri.clone()))?
                    .clone()))
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. } => {
                self.selected(key)?;
                ModuleCompletion::Capabilities(Ok(*self.capabilities.get(&key.uri)?))
            }
            ModuleRequestKind::HasElement {
                key: Some(key),
                element_uri,
                ..
            } => {
                self.selected(key)?;
                ModuleCompletion::HasElement(Ok(*self
                    .elements
                    .get(&(key.uri.clone(), element_uri.clone()))
                    .or_else(|| self.element_defaults.get(&key.uri))?))
            }
            ModuleRequestKind::Resolve {
                key,
                resolved_key_id,
                ..
            }
            | ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => {
                let index = self.selected(key)?.config;
                let config = &self.configs[index];
                self.resolved.insert(*resolved_key_id, key.id);
                ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                    id: *resolved_key_id,
                    key: key.clone(),
                    handle: format!("custom-resolved-{}", resolved_key_id.0).into(),
                    module: ResolvedModule {
                        original_uri: key.uri.clone(),
                        resolved_uri: config.resolved.clone(),
                        source_uri: key.uri.clone(),
                    },
                }))
            }
            ModuleRequestKind::LoadSource { key, .. } => {
                let id = self.resolved.get(&key.id)?;
                assert_eq!(*id, key.key.id);
                assert_eq!(key.handle, format!("custom-resolved-{}", key.id.0).as_str());
                let selected = self.selected(&key.key).expect("resolved original key");
                let config = &self.configs[selected.config];
                let source = if config.source_mode == "state-at-load" {
                    &self.state
                } else {
                    &selected.source
                };
                ModuleCompletion::Source(
                    self.sources
                        .get(source)
                        .expect("source state")
                        .clone()
                        .map(|text| ModuleSource {
                            text: text.into(),
                            inferred_name: "ignored-selected-key-infers-name".into(),
                        }),
                )
            }
            _ => return None,
        })
    }
}
